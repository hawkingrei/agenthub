self.addEventListener("push", (event) => {
  let data = {};
  try {
    data = event.data ? event.data.json() : {};
  } catch {
    data = {};
  }

  const title = "AgentHub";
  const body =
    data.type === "agent_completed"
      ? `Agent ${data.agent_id} completed`
      : "You have a new notification";

  event.waitUntil(
    self.registration.showNotification(title, {
      body,
      data,
    })
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  event.waitUntil(self.clients.openWindow("/"));
});
