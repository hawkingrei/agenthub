use serde_json::{Value, json};

use crate::team::{TEAM_RUN_STATUS_VALUES, TEAM_STEP_STATUS_VALUES};

pub(super) fn openapi_spec() -> Value {
    json!({
      "openapi": "3.0.3",
      "info": {
        "title": "AgentHub HTTP API",
        "version": "0.1.0",
        "description": "AgentHub API contract. Current OpenAPI focus covers Team workbench endpoints and can be incrementally extended."
      },
      "servers": [
        { "url": "/" }
      ],
      "tags": [
        { "name": "openapi", "description": "OpenAPI discovery endpoints" },
        { "name": "agents", "description": "Agent runtime resources" },
        { "name": "teams", "description": "Team definitions, runs, steps, and actor mailbox" }
      ],
      "components": {
        "securitySchemes": {
          "bearerAuth": {
            "type": "http",
            "scheme": "bearer",
            "bearerFormat": "JWT"
          }
        },
        "schemas": {
          "ApiError": {
            "type": "object",
            "required": ["error"],
            "properties": {
              "error": { "type": "string" }
            }
          },
          "TeamDefinitionRecord": {
            "type": "object",
            "required": ["id", "name", "spec", "created_at", "updated_at"],
            "properties": {
              "id": { "type": "string" },
              "name": { "type": "string" },
              "description": { "type": ["string", "null"] },
              "spec": { "type": "object", "additionalProperties": true },
              "created_at": { "type": "integer", "format": "int64" },
              "updated_at": { "type": "integer", "format": "int64" }
            }
          },
          "TeamPromptDefaultsResponse": {
            "type": "object",
            "required": ["coordinator_prompt", "worker_prompt"],
            "properties": {
              "coordinator_prompt": { "type": "string" },
              "worker_prompt": { "type": "string" }
            }
          },
          "TeamRunRecord": {
            "type": "object",
            "required": ["id", "team_id", "context_id", "status", "input", "created_at"],
            "properties": {
              "id": { "type": "string" },
              "team_id": { "type": "string" },
              "context_id": { "type": "string" },
              "status": {
                "type": "string",
                "enum": TEAM_RUN_STATUS_VALUES
              },
              "input": { "type": "object", "additionalProperties": true },
              "summary": { "type": ["string", "null"] },
              "created_at": { "type": "integer", "format": "int64" },
              "started_at": { "type": ["integer", "null"], "format": "int64" },
              "ended_at": { "type": ["integer", "null"], "format": "int64" }
            }
          },
          "TeamRunEventRecord": {
            "type": "object",
            "required": ["event_id", "run_id", "event_type", "ts", "payload"],
            "properties": {
              "event_id": { "type": "integer", "format": "int64" },
              "run_id": { "type": "string" },
              "step_id": { "type": ["string", "null"] },
              "event_type": { "type": "string" },
              "ts": { "type": "integer", "format": "int64" },
              "payload": { "type": "object", "additionalProperties": true }
            }
          },
          "TeamStepRecord": {
            "type": "object",
            "required": [
              "id", "run_id", "step_key", "member_id", "status", "attempt", "depends_on"
            ],
            "properties": {
              "id": { "type": "string" },
              "run_id": { "type": "string" },
              "step_key": { "type": "string" },
              "member_id": { "type": "string" },
              "runtime_handle_id": { "type": ["string", "null"] },
              "remote_task_id": { "type": ["string", "null"] },
              "status": {
                "type": "string",
                "enum": TEAM_STEP_STATUS_VALUES
              },
              "attempt": { "type": "integer", "format": "int64" },
              "depends_on": { "type": "array", "items": { "type": "string" } },
              "input": { "type": ["object", "null"], "additionalProperties": true },
              "output": { "type": ["object", "null"], "additionalProperties": true },
              "error_text": { "type": ["string", "null"] },
              "started_at": { "type": ["integer", "null"], "format": "int64" },
              "ended_at": { "type": ["integer", "null"], "format": "int64" }
            }
          },
          "TeamActorMessageRecord": {
            "type": "object",
            "required": [
              "message_id", "run_id", "from_actor_id", "from_actor_kind", "to_actor_id", "to_actor_kind", "channel", "transport", "payload", "status", "created_at"
            ],
            "properties": {
              "message_id": { "type": "integer", "format": "int64" },
              "run_id": { "type": "string" },
              "from_actor_id": { "type": "string" },
              "from_actor_kind": { "type": "string", "enum": ["agent", "human"] },
              "to_actor_id": { "type": "string" },
              "to_actor_kind": { "type": "string", "enum": ["agent", "human"] },
              "channel": { "type": "string" },
              "transport": { "type": "string", "enum": ["local", "remote"] },
              "route": { "type": ["object", "null"], "additionalProperties": true },
              "payload": { "type": "object", "additionalProperties": true },
              "status": { "type": "string", "enum": ["pending", "delivered", "dead_letter"] },
              "created_at": { "type": "integer", "format": "int64" },
              "delivered_at": { "type": ["integer", "null"], "format": "int64" }
            }
          },
          "TeamMailboxSnapshot": {
            "type": "object",
            "required": ["pending", "delivered", "dead_letter", "recent_messages"],
            "properties": {
              "pending": { "type": "integer", "format": "int64" },
              "delivered": { "type": "integer", "format": "int64" },
              "dead_letter": { "type": "integer", "format": "int64" },
              "recent_messages": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/TeamActorMessageRecord" }
              }
            }
          },
          "TeamMemberSnapshot": {
            "type": "object",
            "required": [
              "member_id",
              "role",
              "skills",
              "pending_inbox_count",
              "status"
            ],
            "properties": {
              "member_id": { "type": "string" },
              "role": { "type": "string", "enum": ["coordinator", "worker"] },
              "model": { "type": ["string", "null"] },
              "prompt": { "type": ["string", "null"] },
              "skills": { "type": "array", "items": { "type": "string" } },
              "pending_inbox_count": { "type": "integer", "format": "int64" },
              "status": { "type": "string" },
              "latest_step": {
                "allOf": [{ "$ref": "#/components/schemas/TeamStepRecord" }],
                "nullable": true
              },
              "session_id": { "type": ["string", "null"] },
              "session_status": { "type": ["string", "null"] }
            }
          },
          "TeamRunSnapshotResponse": {
            "type": "object",
            "required": [
              "run",
              "team",
              "members",
              "steps",
              "latest_events",
              "mailbox"
            ],
            "properties": {
              "run": { "$ref": "#/components/schemas/TeamRunRecord" },
              "team": { "$ref": "#/components/schemas/TeamDefinitionRecord" },
              "coordinator_member_id": { "type": ["string", "null"] },
              "members": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/TeamMemberSnapshot" }
              },
              "steps": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/TeamStepRecord" }
              },
              "latest_events": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/TeamRunEventRecord" }
              },
              "mailbox": { "$ref": "#/components/schemas/TeamMailboxSnapshot" }
            }
          },
          "CreateTeamRequest": {
            "type": "object",
            "required": ["name", "spec"],
            "properties": {
              "name": { "type": "string" },
              "description": { "type": ["string", "null"] },
              "spec": { "type": "object", "additionalProperties": true }
            }
          },
          "TeamChannelRecord": {
            "type": "object",
            "required": [
              "team_id",
              "channel_id",
              "task_id",
              "conversation_id",
              "created_by_actor_id",
              "created_at",
              "updated_at"
            ],
            "properties": {
              "team_id": { "type": "string" },
              "channel_id": { "type": "string" },
              "task_id": { "type": "string" },
              "conversation_id": { "type": "string" },
              "description": { "type": ["string", "null"] },
              "created_by_actor_id": { "type": "string" },
              "created_at": { "type": "integer", "format": "int64" },
              "updated_at": { "type": "integer", "format": "int64" }
            }
          },
          "CreateTeamChannelRequest": {
            "type": "object",
            "required": ["channel_id"],
            "properties": {
              "channel_id": { "type": "string" },
              "description": { "type": ["string", "null"] }
            }
          },
          "TeamUploadRequest": {
            "type": "object",
            "required": ["file_name", "content_type", "bytes_base64"],
            "properties": {
              "file_name": { "type": "string" },
              "content_type": { "type": "string" },
              "bytes_base64": { "type": "string", "format": "byte" },
              "expected_size_bytes": { "type": ["integer", "null"], "format": "int64" },
              "expected_sha256": { "type": ["string", "null"] }
            }
          },
          "UploadSessionPrepareRequest": {
            "type": "object",
            "required": ["file_name", "content_type", "object_kind", "expected_size_bytes"],
            "properties": {
              "file_name": { "type": "string" },
              "content_type": { "type": "string" },
              "object_kind": { "type": "string", "enum": ["object", "image"] },
              "expected_size_bytes": { "type": "integer", "format": "int64" },
              "expected_sha256": { "type": ["string", "null"] },
              "ttl_seconds": { "type": ["integer", "null"], "format": "int64" }
            }
          },
          "UploadSessionDirectWriteRequest": {
            "type": "object",
            "properties": {
              "expires_in_seconds": { "type": ["integer", "null"], "format": "int64" }
            }
          },
          "UploadSessionDirectWriteResponse": {
            "type": "object",
            "required": ["session_id", "object_key", "method", "url", "headers", "expires_at"],
            "properties": {
              "session_id": { "type": "string" },
              "object_key": { "type": "string" },
              "method": { "type": "string" },
              "url": { "type": "string" },
              "headers": {
                "type": "array",
                "items": {
                  "type": "object",
                  "required": ["name", "value"],
                  "properties": {
                    "name": { "type": "string" },
                    "value": { "type": "string" }
                  }
                }
              },
              "expires_at": { "type": "integer", "format": "int64" }
            }
          },
          "UploadSessionMultipartUploadResponse": {
            "type": "object",
            "required": ["session_id", "object_key", "upload_id"],
            "properties": {
              "session_id": { "type": "string" },
              "object_key": { "type": "string" },
              "upload_id": { "type": "string" }
            }
          },
          "UploadSessionMultipartPartWriteRequest": {
            "type": "object",
            "required": ["upload_id"],
            "properties": {
              "upload_id": { "type": "string" },
              "expires_in_seconds": { "type": ["integer", "null"], "format": "int64" }
            }
          },
          "UploadSessionMultipartPartWriteResponse": {
            "type": "object",
            "required": ["session_id", "object_key", "upload_id", "part_number", "method", "url", "headers", "expires_at"],
            "properties": {
              "session_id": { "type": "string" },
              "object_key": { "type": "string" },
              "upload_id": { "type": "string" },
              "part_number": { "type": "integer", "format": "int32" },
              "method": { "type": "string" },
              "url": { "type": "string" },
              "headers": {
                "type": "array",
                "items": {
                  "type": "object",
                  "required": ["name", "value"],
                  "properties": {
                    "name": { "type": "string" },
                    "value": { "type": "string" }
                  }
                }
              },
              "expires_at": { "type": "integer", "format": "int64" }
            }
          },
          "UploadSessionMultipartCompleteRequest": {
            "type": "object",
            "required": ["upload_id", "parts"],
            "properties": {
              "upload_id": { "type": "string" },
              "parts": {
                "type": "array",
                "items": {
                  "type": "object",
                  "required": ["part_number", "etag"],
                  "properties": {
                    "part_number": { "type": "integer", "format": "int32", "minimum": 1 },
                    "etag": { "type": "string" }
                  }
                }
              }
            }
          },
          "UploadSessionMultipartAbortRequest": {
            "type": "object",
            "required": ["upload_id"],
            "properties": {
              "upload_id": { "type": "string" }
            }
          },
          "ObjectUploadRecord": {
            "type": "object",
            "required": [
              "id",
              "owner_scope",
              "backend",
              "object_key",
              "original_filename",
              "content_type",
              "size_bytes",
              "sha256",
              "created_by_actor_id",
              "publish_state",
              "created_at"
            ],
            "properties": {
              "id": { "type": "string" },
              "owner_scope": { "type": "string" },
              "backend": { "type": "string" },
              "object_key": { "type": "string" },
              "original_filename": { "type": "string" },
              "content_type": { "type": "string" },
              "size_bytes": { "type": "integer", "format": "int64" },
              "sha256": { "type": "string" },
              "public_url": { "type": ["string", "null"] },
              "created_by_actor_id": { "type": "string" },
              "publish_state": { "type": "string" },
              "created_at": { "type": "integer", "format": "int64" },
              "published_at": { "type": ["integer", "null"], "format": "int64" },
              "cleanup_after": { "type": ["integer", "null"], "format": "int64" }
            }
          },
          "ObjectUploadSessionRecord": {
            "type": "object",
            "required": [
              "id",
              "owner_scope",
              "backend",
              "object_key",
              "original_filename",
              "content_type",
              "object_kind",
              "expected_size_bytes",
              "created_by_actor_id",
              "status",
              "created_at",
              "expires_at"
            ],
            "properties": {
              "id": { "type": "string" },
              "owner_scope": { "type": "string" },
              "backend": { "type": "string" },
              "object_key": { "type": "string" },
              "original_filename": { "type": "string" },
              "content_type": { "type": "string" },
              "object_kind": { "type": "string" },
              "expected_size_bytes": { "type": "integer", "format": "int64" },
              "expected_sha256": { "type": ["string", "null"] },
              "created_by_actor_id": { "type": "string" },
              "status": { "type": "string" },
              "created_at": { "type": "integer", "format": "int64" },
              "expires_at": { "type": "integer", "format": "int64" },
              "completed_at": { "type": ["integer", "null"], "format": "int64" },
              "canceled_at": { "type": ["integer", "null"], "format": "int64" },
              "published_upload_id": { "type": ["string", "null"] }
            }
          },
          "ObjectUploadSessionPartRecord": {
            "type": "object",
            "required": [
              "session_id",
              "part_number",
              "object_key",
              "size_bytes",
              "sha256",
              "uploaded_at"
            ],
            "properties": {
              "session_id": { "type": "string" },
              "part_number": { "type": "integer", "format": "int64" },
              "object_key": { "type": "string" },
              "size_bytes": { "type": "integer", "format": "int64" },
              "sha256": { "type": "string" },
              "uploaded_at": { "type": "integer", "format": "int64" }
            }
          },
          "CreateTeamRunRequest": {
            "type": "object",
            "properties": {
              "context_id": { "type": ["string", "null"] },
              "input": { "type": ["object", "null"], "additionalProperties": true }
            }
          },
          "SubmitTeamRunStepRequest": {
            "type": "object",
            "required": ["step_key", "member_id"],
            "properties": {
              "step_key": { "type": "string" },
              "member_id": { "type": "string" },
              "depends_on": { "type": "array", "items": { "type": "string" } },
              "input": { "type": ["object", "null"], "additionalProperties": true }
            }
          },
          "SendTeamRunMessageRequest": {
            "type": "object",
            "required": ["from_actor_id", "to_actor_id", "payload"],
            "properties": {
              "from_actor_id": { "type": "string" },
              "to_actor_id": { "type": "string" },
              "channel": { "type": ["string", "null"] },
              "transport": { "type": ["string", "null"], "enum": ["local", "remote", null] },
              "route": { "type": ["object", "null"], "additionalProperties": true },
              "payload": { "type": "object", "additionalProperties": true },
              "idempotency_key": { "type": ["string", "null"] }
            }
          }
        }
      },
      "security": [
        { "bearerAuth": [] }
      ],
      "paths": {
        "/api/openapi.json": {
          "get": {
            "tags": ["openapi"],
            "summary": "Get OpenAPI JSON",
            "responses": {
              "200": {
                "description": "OpenAPI document",
                "content": {
                  "application/json": {
                    "schema": { "type": "object", "additionalProperties": true }
                  }
                }
              },
              "401": {
                "description": "Unauthorized",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiError" }
                  }
                }
              }
            }
          }
        },
        "/api/openapi/docs": {
          "get": {
            "tags": ["openapi"],
            "summary": "Get OpenAPI docs page",
            "security": [],
            "responses": {
              "200": {
                "description": "HTML docs page",
                "content": {
                  "text/html": {
                    "schema": { "type": "string" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads": {
          "post": {
            "tags": ["agents"],
            "summary": "Upload agent object",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/TeamUploadRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads/sessions": {
          "post": {
            "tags": ["agents"],
            "summary": "Prepare agent upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionPrepareRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Prepared upload session",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads/sessions/{session_id}/cancel": {
          "post": {
            "tags": ["agents"],
            "summary": "Cancel agent upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Canceled upload session",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads/sessions/{session_id}/complete": {
          "post": {
            "tags": ["agents"],
            "summary": "Complete agent upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/octet-stream": {
                  "schema": { "type": "string", "format": "binary" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads/sessions/{session_id}/direct-write": {
          "post": {
            "tags": ["agents"],
            "summary": "Prepare agent direct upload session write",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionDirectWriteRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Prepared direct upload write request",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UploadSessionDirectWriteResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads/sessions/{session_id}/complete-direct": {
          "post": {
            "tags": ["agents"],
            "summary": "Complete agent direct upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads/sessions/{session_id}/multipart": {
          "post": {
            "tags": ["agents"],
            "summary": "Initiate agent multipart upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Initiated multipart upload",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UploadSessionMultipartUploadResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads/sessions/{session_id}/multipart/parts/{part_number}": {
          "post": {
            "tags": ["agents"],
            "summary": "Prepare agent multipart upload part",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "part_number", "in": "path", "required": true, "schema": { "type": "integer", "format": "int32", "minimum": 1 } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionMultipartPartWriteRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Prepared multipart upload part write request",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UploadSessionMultipartPartWriteResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads/sessions/{session_id}/multipart/complete": {
          "post": {
            "tags": ["agents"],
            "summary": "Complete agent multipart upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionMultipartCompleteRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads/sessions/{session_id}/multipart/abort": {
          "post": {
            "tags": ["agents"],
            "summary": "Abort agent multipart upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionMultipartAbortRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Canceled upload session",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads/sessions/{session_id}/parts/{part_number}": {
          "post": {
            "tags": ["agents"],
            "summary": "Upload agent session part",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "part_number", "in": "path", "required": true, "schema": { "type": "integer", "format": "int32", "minimum": 1 } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/octet-stream": {
                  "schema": { "type": "string", "format": "binary" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Uploaded session part metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionPartRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/uploads/sessions/{session_id}/complete-parts": {
          "post": {
            "tags": ["agents"],
            "summary": "Complete agent upload session from parts",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/agents/{id}/images": {
          "post": {
            "tags": ["agents"],
            "summary": "Upload agent image",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/TeamUploadRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published image metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams": {
          "get": {
            "tags": ["teams"],
            "summary": "List teams",
            "responses": {
              "200": {
                "description": "Team definitions",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/TeamDefinitionRecord" }
                    }
                  }
                }
              }
            }
          },
          "post": {
            "tags": ["teams"],
            "summary": "Create team",
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/CreateTeamRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Created team",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamDefinitionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/prompt_defaults": {
          "get": {
            "tags": ["teams"],
            "summary": "Get default Team prompts",
            "responses": {
              "200": {
                "description": "Default coordinator and worker prompts",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamPromptDefaultsResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}": {
          "get": {
            "tags": ["teams"],
            "summary": "Get team",
            "parameters": [
              {
                "name": "id",
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
              }
            ],
            "responses": {
              "200": {
                "description": "Team definition",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamDefinitionRecord" }
                  }
                }
              }
            }
          },
          "delete": {
            "tags": ["teams"],
            "summary": "Delete team",
            "parameters": [
              {
                "name": "id",
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
              }
            ],
            "responses": {
              "200": {
                "description": "Deleted team definition",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamDefinitionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads": {
          "post": {
            "tags": ["teams"],
            "summary": "Upload team object",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/TeamUploadRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads/sessions": {
          "post": {
            "tags": ["teams"],
            "summary": "Prepare team upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionPrepareRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Prepared upload session",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads/sessions/{session_id}/cancel": {
          "post": {
            "tags": ["teams"],
            "summary": "Cancel team upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Canceled upload session",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads/sessions/{session_id}/complete": {
          "post": {
            "tags": ["teams"],
            "summary": "Complete team upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/octet-stream": {
                  "schema": { "type": "string", "format": "binary" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads/sessions/{session_id}/direct-write": {
          "post": {
            "tags": ["teams"],
            "summary": "Prepare team direct upload session write",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionDirectWriteRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Prepared direct upload write request",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UploadSessionDirectWriteResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads/sessions/{session_id}/complete-direct": {
          "post": {
            "tags": ["teams"],
            "summary": "Complete team direct upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads/sessions/{session_id}/multipart": {
          "post": {
            "tags": ["teams"],
            "summary": "Initiate team multipart upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Initiated multipart upload",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UploadSessionMultipartUploadResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads/sessions/{session_id}/multipart/parts/{part_number}": {
          "post": {
            "tags": ["teams"],
            "summary": "Prepare team multipart upload part",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "part_number", "in": "path", "required": true, "schema": { "type": "integer", "format": "int32", "minimum": 1 } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionMultipartPartWriteRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Prepared multipart upload part write request",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UploadSessionMultipartPartWriteResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads/sessions/{session_id}/multipart/complete": {
          "post": {
            "tags": ["teams"],
            "summary": "Complete team multipart upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionMultipartCompleteRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads/sessions/{session_id}/multipart/abort": {
          "post": {
            "tags": ["teams"],
            "summary": "Abort team multipart upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionMultipartAbortRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Canceled upload session",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads/sessions/{session_id}/parts/{part_number}": {
          "post": {
            "tags": ["teams"],
            "summary": "Upload team session part",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "part_number", "in": "path", "required": true, "schema": { "type": "integer", "format": "int32", "minimum": 1 } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/octet-stream": {
                  "schema": { "type": "string", "format": "binary" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Uploaded session part metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionPartRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/uploads/sessions/{session_id}/complete-parts": {
          "post": {
            "tags": ["teams"],
            "summary": "Complete team upload session from parts",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/images": {
          "post": {
            "tags": ["teams"],
            "summary": "Upload team image",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/TeamUploadRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published image metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads": {
          "post": {
            "tags": ["teams"],
            "summary": "Upload team task object",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/TeamUploadRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads/sessions": {
          "post": {
            "tags": ["teams"],
            "summary": "Prepare team task upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionPrepareRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Prepared upload session",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/cancel": {
          "post": {
            "tags": ["teams"],
            "summary": "Cancel team task upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Canceled upload session",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/complete": {
          "post": {
            "tags": ["teams"],
            "summary": "Complete team task upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/octet-stream": {
                  "schema": { "type": "string", "format": "binary" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/direct-write": {
          "post": {
            "tags": ["teams"],
            "summary": "Prepare team task direct upload session write",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionDirectWriteRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Prepared direct upload write request",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UploadSessionDirectWriteResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/complete-direct": {
          "post": {
            "tags": ["teams"],
            "summary": "Complete team task direct upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/multipart": {
          "post": {
            "tags": ["teams"],
            "summary": "Initiate team task multipart upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Initiated multipart upload",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UploadSessionMultipartUploadResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/multipart/parts/{part_number}": {
          "post": {
            "tags": ["teams"],
            "summary": "Prepare team task multipart upload part",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "part_number", "in": "path", "required": true, "schema": { "type": "integer", "format": "int32", "minimum": 1 } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionMultipartPartWriteRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Prepared multipart upload part write request",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UploadSessionMultipartPartWriteResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/multipart/complete": {
          "post": {
            "tags": ["teams"],
            "summary": "Complete team task multipart upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionMultipartCompleteRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/multipart/abort": {
          "post": {
            "tags": ["teams"],
            "summary": "Abort team task multipart upload session",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UploadSessionMultipartAbortRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Canceled upload session",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/parts/{part_number}": {
          "post": {
            "tags": ["teams"],
            "summary": "Upload team task session part",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "part_number", "in": "path", "required": true, "schema": { "type": "integer", "format": "int32", "minimum": 1 } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/octet-stream": {
                  "schema": { "type": "string", "format": "binary" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Uploaded session part metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadSessionPartRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/complete-parts": {
          "post": {
            "tags": ["teams"],
            "summary": "Complete team task upload session from parts",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "session_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Published object metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/tasks/{task_id}/images": {
          "post": {
            "tags": ["teams"],
            "summary": "Upload team task image",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "task_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/TeamUploadRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Published image metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ObjectUploadRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/channels": {
          "get": {
            "tags": ["teams"],
            "summary": "List team channels",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Team channels",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/TeamChannelRecord" }
                    }
                  }
                }
              }
            }
          },
          "post": {
            "tags": ["teams"],
            "summary": "Create team channel",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/CreateTeamChannelRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Created team channel",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamChannelRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/channels/{channel_id}": {
          "delete": {
            "tags": ["teams"],
            "summary": "Delete team channel",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              {
                "name": "channel_id",
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
              }
            ],
            "responses": {
              "200": {
                "description": "Deleted team channel",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamChannelRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/runs": {
          "get": {
            "tags": ["teams"],
            "summary": "List runs for a team",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "limit", "in": "query", "schema": { "type": "integer", "minimum": 1, "maximum": 500 } },
              {
                "name": "status",
                "in": "query",
                "schema": {
                  "type": "string",
                  "enum": TEAM_RUN_STATUS_VALUES
                }
              },
              { "name": "before_created_at", "in": "query", "schema": { "type": "integer", "format": "int64" } }
            ],
            "responses": {
              "200": {
                "description": "Team runs",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/TeamRunRecord" }
                    }
                  }
                }
              }
            }
          },
          "post": {
            "tags": ["teams"],
            "summary": "Create run for a team",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/CreateTeamRunRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Created run",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamRunRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}": {
          "get": {
            "tags": ["teams"],
            "summary": "Get run",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Run record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamRunRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/cancel": {
          "post": {
            "tags": ["teams"],
            "summary": "Cancel run",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Canceled run",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamRunRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/resume": {
          "post": {
            "tags": ["teams"],
            "summary": "Resume run",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Resumed run",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamRunRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/restart": {
          "post": {
            "tags": ["teams"],
            "summary": "Restart run",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Restarted run",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamRunRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/snapshot": {
          "get": {
            "tags": ["teams"],
            "summary": "Get run snapshot",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "event_limit", "in": "query", "schema": { "type": "integer", "minimum": 1, "maximum": 20 } },
              { "name": "message_limit", "in": "query", "schema": { "type": "integer", "minimum": 1, "maximum": 20 } }
            ],
            "responses": {
              "200": {
                "description": "Run snapshot",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamRunSnapshotResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/events": {
          "get": {
            "tags": ["teams"],
            "summary": "List run events",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "limit", "in": "query", "schema": { "type": "integer", "minimum": 1, "maximum": 20 } },
              { "name": "before_id", "in": "query", "schema": { "type": "integer", "format": "int64" } }
            ],
            "responses": {
              "200": {
                "description": "Run events",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/TeamRunEventRecord" }
                    }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps": {
          "get": {
            "tags": ["teams"],
            "summary": "List run steps",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Run steps",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/TeamStepRecord" }
                    }
                  }
                }
              }
            }
          },
          "post": {
            "tags": ["teams"],
            "summary": "Submit run step",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/SubmitTeamRunStepRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps/{step_id}/start": {
          "post": {
            "tags": ["teams"],
            "summary": "Start run step",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "step_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps/{step_id}/complete": {
          "post": {
            "tags": ["teams"],
            "summary": "Complete run step",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "step_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps/{step_id}/fail": {
          "post": {
            "tags": ["teams"],
            "summary": "Fail run step",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "step_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps/{step_id}/input_required": {
          "post": {
            "tags": ["teams"],
            "summary": "Set run step input required",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "step_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps/{step_id}/resume": {
          "post": {
            "tags": ["teams"],
            "summary": "Resume run step",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "step_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/messages/send": {
          "post": {
            "tags": ["teams"],
            "summary": "Send actor message",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/SendTeamRunMessageRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Actor message record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamActorMessageRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/messages/inbox": {
          "get": {
            "tags": ["teams"],
            "summary": "List actor inbox",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "actor_id", "in": "query", "required": true, "schema": { "type": "string" } },
              { "name": "limit", "in": "query", "schema": { "type": "integer", "minimum": 1, "maximum": 1000 } },
              { "name": "after_id", "in": "query", "schema": { "type": "integer", "format": "int64" } },
              { "name": "include_delivered", "in": "query", "schema": { "type": "boolean" } }
            ],
            "responses": {
              "200": {
                "description": "Actor inbox",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/TeamActorMessageRecord" }
                    }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/messages/{message_id}/ack": {
          "post": {
            "tags": ["teams"],
            "summary": "Acknowledge actor message",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "message_id", "in": "path", "required": true, "schema": { "type": "integer", "format": "int64" } }
            ],
            "responses": {
              "200": {
                "description": "Actor message record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamActorMessageRecord" }
                  }
                }
              }
            }
          }
        }
      }
    })
}
