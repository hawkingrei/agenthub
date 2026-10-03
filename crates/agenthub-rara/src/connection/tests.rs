use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, DuplexStream};

use super::*;
use crate::{ControlEnvelope, Provenance, ReceiptCapability};

fn hello() -> Handshake {
    let fixture: Value =
        serde_json::from_str(include_str!("../../fixtures/stdio-v1.json")).unwrap();
    serde_json::from_value(fixture["frames"][0]["payload"].clone()).unwrap()
}

struct Peer {
    input: BufReader<DuplexStream>,
    output: DuplexStream,
}

impl Peer {
    async fn send(&mut self, frame: Value) {
        let mut bytes = serde_json::to_vec(&frame).unwrap();
        bytes.push(b'\n');
        self.output.write_all(&bytes).await.unwrap();
    }

    async fn receive(&mut self) -> Value {
        let mut line = String::new();
        assert_ne!(self.input.read_line(&mut line).await.unwrap(), 0);
        serde_json::from_str(&line).unwrap()
    }

    async fn ack(&mut self, id: &str) {
        self.send(json!({"type":"ack","payload":{
            "runtime_id":"runtime-fixture","request_id":id,
            "result":{"status":"accepted","session_id":null,"turn_id":null,"last_sequence":null}
        }}))
        .await;
    }

    async fn complete(&mut self, id: &str) {
        self.send(json!({"type":"shutdown_complete","payload":{
            "runtime_id":"runtime-fixture","request_id":id
        }}))
        .await;
    }
}

async fn open(handshake: Handshake, options: ConnectionOptions) -> (Connection, Peer) {
    let (input, output_peer) = tokio::io::duplex(crate::MAX_FRAME_BYTES * 2);
    let (output, input_peer) = tokio::io::duplex(crate::MAX_FRAME_BYTES * 2);
    let mut peer = Peer {
        input: BufReader::new(input_peer),
        output: output_peer,
    };
    peer.send(serde_json::to_value(ServerFrame::Handshake(handshake)).unwrap())
        .await;
    let connection = Connection::open(BufReader::new(input), output, options)
        .await
        .unwrap();
    (connection, peer)
}

fn create(id: &str) -> ClientFrame {
    ClientFrame::Control {
        runtime_id: "runtime-fixture".into(),
        envelope: ControlEnvelope {
            request_id: id.into(),
            provenance: Provenance::new(None),
            request: json!({"type":"session","payload":{"type":"create_session"}}),
        },
        expected_turn_id: None,
    }
}

fn event(sequence: u64) -> Value {
    json!({"type":"event","payload":{
        "runtime_id":"runtime-fixture","session_id":"session-fixture",
        "event":{"event_id":format!("event-{sequence}"),"provenance":{},"sequence":sequence,
        "event":{"type":"session","payload":{"type":"created"}}}
    }})
}

fn send_request(
    client: &Client,
    frame: ClientFrame,
) -> tokio::task::JoinHandle<Result<Acknowledgement, ConnectionError>> {
    let client = client.clone();
    tokio::spawn(async move { client.request(frame).await })
}

fn start_shutdown(
    client: &Client,
) -> tokio::task::JoinHandle<Result<ShutdownReceipt, ConnectionError>> {
    let client = client.clone();
    tokio::spawn(async move { client.shutdown("shutdown-1".into()).await })
}

#[tokio::test(start_paused = true)]
async fn startup_requires_stdout_handshake_and_closes_both_pipes_on_failure() {
    let options = ConnectionOptions::default();
    let (input, _silent_output) = tokio::io::duplex(4096);
    let (output, mut peer_input) = tokio::io::duplex(4096);
    assert!(matches!(
        Connection::open(BufReader::new(input), output, options).await,
        Err(ConnectionError::StartupTimeout)
    ));
    assert_eq!(peer_input.read(&mut [0]).await.unwrap(), 0);

    for first in [
        event(1),
        json!({"type":"handshake","payload":{"private":"credential"}}),
        {
            let mut missing = hello();
            missing
                .request_methods
                .retain(|m| m != "input.answer_shell");
            serde_json::to_value(ServerFrame::Handshake(missing)).unwrap()
        },
    ] {
        let mut bytes = serde_json::to_vec(&first).unwrap();
        bytes.push(b'\n');
        let (output, mut peer_input) = tokio::io::duplex(4096);
        let error = Connection::open(std::io::Cursor::new(bytes), output, options)
            .await
            .err()
            .unwrap();
        assert!(!error.to_string().contains("credential"));
        assert_eq!(peer_input.read(&mut [0]).await.unwrap(), 0);
    }
    let (input, _) = tokio::io::duplex(1);
    assert!(matches!(
        Connection::open(
            BufReader::new(input),
            tokio::io::sink(),
            ConnectionOptions {
                startup_timeout: Duration::ZERO,
                ..options
            }
        )
        .await,
        Err(ConnectionError::InvalidOptions)
    ));
}

#[tokio::test(start_paused = true)]
async fn ack_is_not_shutdown_completion_and_stdin_stays_open_through_drain() {
    let (mut connection, mut peer) = open(hello(), ConnectionOptions::default()).await;
    let pending = send_request(&connection.client, create("create-1"));
    assert_eq!(
        peer.receive().await["payload"]["envelope"]["request_id"],
        "create-1"
    );
    peer.ack("create-1").await;
    assert!(matches!(
        pending.await.unwrap().unwrap().result,
        RequestResult::Accepted { .. }
    ));
    assert_eq!(connection.client.status(), ConnectionStatus::Running);
    let shutdown = start_shutdown(&connection.client);
    assert_eq!(peer.receive().await["type"], "shutdown");
    peer.ack("shutdown-1").await;
    peer.send(event(1)).await;
    assert!(matches!(
        connection.output.recv().await,
        Some(OutputFrame::Event(_))
    ));
    assert!(!shutdown.is_finished());
    assert_eq!(
        connection.client.request(create("too-late")).await,
        Err(ConnectionError::Closing)
    );
    // The child can finish while its stdin remains open. EOF alone is not the signal.
    assert!(
        timeout(Duration::from_millis(1), peer.input.read(&mut [0]))
            .await
            .is_err()
    );
    peer.complete("shutdown-1").await;
    tokio::task::yield_now().await;
    assert!(!shutdown.is_finished());
    peer.output.shutdown().await.unwrap();
    let receipt = shutdown.await.unwrap().unwrap();
    assert_eq!(receipt.request_id, "shutdown-1");
    assert_eq!(connection.client.closed().await, Ok(receipt));
    assert_eq!(peer.input.read(&mut [0]).await.unwrap(), 0);
    assert_eq!(connection.output.recv().await, None);
}

#[tokio::test(start_paused = true)]
async fn shutdown_requires_matching_accepted_ack_completion_and_clean_eof() {
    for scenario in [
        "eof",
        "ack_only",
        "complete_without_ack",
        "wrong_id",
        "trailing",
        "no_eof",
        "reject",
        "queued",
    ] {
        let (connection, mut peer) = open(hello(), ConnectionOptions::default()).await;
        let shutdown = start_shutdown(&connection.client);
        peer.receive().await;
        match scenario {
            "eof" => peer.output.shutdown().await.unwrap(),
            "ack_only" => {
                peer.ack("shutdown-1").await;
                peer.output.shutdown().await.unwrap();
            }
            "complete_without_ack" => peer.complete("shutdown-1").await,
            "wrong_id" => {
                peer.ack("shutdown-1").await;
                peer.complete("wrong").await;
            }
            "trailing" => {
                peer.ack("shutdown-1").await;
                peer.complete("shutdown-1").await;
                peer.send(event(1)).await;
            }
            "no_eof" => {
                peer.ack("shutdown-1").await;
                peer.complete("shutdown-1").await;
            }
            "reject" | "queued" => {
                let result = if scenario == "reject" {
                    json!({"status":"rejected","code":"overloaded","message":"capacity"})
                } else {
                    json!({"status":"queued","session_id":"session-fixture"})
                };
                peer.send(json!({"type":"ack","payload":{
                    "runtime_id":"runtime-fixture","request_id":"shutdown-1","result":result
                }}))
                .await;
            }
            _ => unreachable!(),
        }
        let result = shutdown.await.unwrap();
        assert!(result.is_err(), "{scenario}");
        if scenario == "no_eof" {
            assert_eq!(result, Err(ConnectionError::ShutdownTimeout));
        }
        assert!(connection.client.closed().await.is_err());
        assert_eq!(peer.input.read(&mut [0]).await.unwrap(), 0);
    }
}

#[tokio::test(start_paused = true)]
async fn runtime_and_request_correlation_fail_closed_without_echoing_frames() {
    for frame in [
        json!({"type":"ack","payload":{"runtime_id":"runtime-fixture","request_id":"unknown",
            "result":{"status":"accepted","session_id":null,"turn_id":null,"last_sequence":null}}}),
        {
            let mut value = event(1);
            value["payload"]["runtime_id"] = json!("other-runtime");
            value
        },
        serde_json::to_value(ServerFrame::Handshake(hello())).unwrap(),
        json!({"type":"replay_gap","payload":{"runtime_id":"runtime-fixture","request_id":"unknown",
            "session_id":"session-fixture","requested_after":0,"oldest_available":2,"latest":4}}),
    ] {
        let (connection, mut peer) = open(hello(), ConnectionOptions::default()).await;
        peer.send(frame).await;
        assert!(matches!(
            connection.client.closed().await,
            Err(ConnectionError::WrongRuntime | ConnectionError::UnexpectedFrame)
        ));
    }
}

#[tokio::test(start_paused = true)]
async fn timeout_after_write_is_unknown_and_never_retried() {
    let (connection, mut peer) = open(hello(), ConnectionOptions::default()).await;
    let pending = send_request(&connection.client, create("uncertain"));
    peer.receive().await;
    assert_eq!(pending.await.unwrap(), Err(ConnectionError::RequestTimeout));
    assert!(connection.client.closed().await.is_err());
    assert_eq!(peer.input.read(&mut [0]).await.unwrap(), 0);
    assert!(
        connection
            .client
            .request(create("replacement"))
            .await
            .is_err()
    );
}

#[tokio::test(start_paused = true)]
async fn cancelled_caller_does_not_retract_or_repeat_the_request() {
    let (connection, mut peer) = open(hello(), ConnectionOptions::default()).await;
    let pending = send_request(&connection.client, create("cancelled-caller"));
    peer.receive().await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    peer.ack("cancelled-caller").await;
    let next = send_request(&connection.client, create("next"));
    assert_eq!(
        peer.receive().await["payload"]["envelope"]["request_id"],
        "next"
    );
    peer.ack("next").await;
    next.await.unwrap().unwrap();
    assert_eq!(
        connection.client.request(create("cancelled-caller")).await,
        Err(ConnectionError::RequestIdReused)
    );
    connection.client.abort();
    assert_eq!(
        connection.client.closed().await,
        Err(ConnectionError::Aborted)
    );
}

#[tokio::test(start_paused = true)]
async fn unsupported_requests_and_capacity_preserve_a_shutdown_receipt() {
    let mut handshake = hello();
    handshake.capabilities.request_receipts = ReceiptCapability::Runtime { max_requests: 2 };
    let (connection, mut peer) = open(handshake, ConnectionOptions::default()).await;
    let mut wrong = create("wrong");
    if let ClientFrame::Control { runtime_id, .. } = &mut wrong {
        *runtime_id = "other".into();
    }
    assert_eq!(
        connection.client.request(wrong).await,
        Err(ConnectionError::WrongRuntime)
    );
    let mut resume = create("resume");
    if let ClientFrame::Control { envelope, .. } = &mut resume {
        envelope.request = json!({"type":"session","payload":{"type":"resume_session","payload":{"session_id":"old"}}});
    }
    assert_eq!(
        connection.client.request(resume).await,
        Err(ConnectionError::UnsupportedMethod)
    );
    let pending = send_request(&connection.client, create("one"));
    peer.receive().await;
    peer.ack("one").await;
    pending.await.unwrap().unwrap();
    assert_eq!(
        connection.client.request(create("one")).await,
        Err(ConnectionError::RequestIdReused)
    );
    assert_eq!(
        connection.client.request(create("two")).await,
        Err(ConnectionError::ReceiptCapacity)
    );
    let shutdown = start_shutdown(&connection.client);
    assert_eq!(peer.receive().await["type"], "shutdown");
    peer.ack("shutdown-1").await;
    peer.complete("shutdown-1").await;
    peer.output.shutdown().await.unwrap();
    shutdown.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn stalled_event_consumer_fails_explicitly_instead_of_silently_dropping() {
    let (mut connection, mut peer) = open(hello(), ConnectionOptions::default()).await;
    for sequence in 1..=(OUTPUT_CAPACITY as u64 + 1) {
        peer.send(event(sequence)).await;
    }
    assert_eq!(
        connection.client.closed().await,
        Err(ConnectionError::OutputStalled)
    );
    for expected in 1..=OUTPUT_CAPACITY as u64 {
        let Some(OutputFrame::Event(frame)) = connection.output.recv().await else {
            panic!("queued event");
        };
        assert_eq!(frame.event.sequence, expected);
    }
    assert_eq!(connection.output.recv().await, None);
}

#[tokio::test(start_paused = true)]
async fn closing_consumer_or_last_client_releases_streams() {
    let (connection, mut peer) = open(hello(), ConnectionOptions::default()).await;
    drop(connection.output);
    assert_eq!(
        connection.client.closed().await,
        Err(ConnectionError::ConsumerClosed)
    );
    assert_eq!(peer.input.read(&mut [0]).await.unwrap(), 0);

    let (mut connection, mut peer) = open(hello(), ConnectionOptions::default()).await;
    drop(connection.client);
    assert_eq!(connection.output.recv().await, None);
    assert_eq!(peer.input.read(&mut [0]).await.unwrap(), 0);
}

#[tokio::test(start_paused = true)]
async fn broken_writer_and_truncated_output_fail_pending_requests() {
    let (connection, peer) = open(hello(), ConnectionOptions::default()).await;
    let Peer {
        input,
        output: _keep_output_open,
    } = peer;
    drop(input);
    assert_eq!(
        connection.client.request(create("broken-write")).await,
        Err(ConnectionError::Protocol(ProtocolError::TransportLost))
    );

    let (connection, mut peer) = open(hello(), ConnectionOptions::default()).await;
    let pending = send_request(&connection.client, create("truncated"));
    peer.receive().await;
    peer.output.write_all(b"{\"type\":").await.unwrap();
    peer.output.shutdown().await.unwrap();
    assert_eq!(
        pending.await.unwrap(),
        Err(ConnectionError::Protocol(ProtocolError::MalformedFrame))
    );
}

#[tokio::test(start_paused = true)]
async fn stalled_stdin_does_not_block_events_or_escape_request_deadline() {
    let (input, output_peer) = tokio::io::duplex(4096);
    let (output, input_peer) = tokio::io::duplex(1);
    let mut peer = Peer {
        input: BufReader::new(input_peer),
        output: output_peer,
    };
    peer.send(serde_json::to_value(ServerFrame::Handshake(hello())).unwrap())
        .await;
    let mut connection =
        Connection::open(BufReader::new(input), output, ConnectionOptions::default())
            .await
            .unwrap();
    let pending = send_request(&connection.client, create("blocked-write"));
    peer.send(event(1)).await;
    assert!(matches!(
        connection.output.recv().await,
        Some(OutputFrame::Event(_))
    ));
    assert_eq!(pending.await.unwrap(), Err(ConnectionError::RequestTimeout));
    assert!(connection.client.closed().await.is_err());
    let mut remainder = Vec::new();
    peer.input.read_to_end(&mut remainder).await.unwrap();
    assert_eq!(remainder, b"{");
}
