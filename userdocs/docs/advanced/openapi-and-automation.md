---
sidebar_position: 2
---

# OpenAPI and Automation

AgentHub exposes authenticated OpenAPI endpoints for integration and automation workflows. This guide covers API usage, client generation, and real-world automation patterns.

## Endpoints

| Endpoint | Description | Auth Required |
|----------|-------------|---------------|
| `/api/openapi.json` | Full OpenAPI specification | Yes |
| `/api/openapi/docs` | Interactive documentation | Yes |

## Quick Start

### Authentication

All API endpoints require Bearer token authentication:

```bash
# Login to get token
TOKEN=$(curl -X POST http://localhost:8080/api/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"secret"}' \
  | jq -r '.token')

# Use token in requests
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/agents
```

### Fetch OpenAPI Spec

```bash
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/openapi.json \
  > agenthub-openapi.json
```

## Client Generation

### TypeScript/JavaScript

Using `openapi-typescript`:

```bash
# Install
npm install -g openapi-typescript

# Generate types
openapi-typescript agenthub-openapi.json -o agenthub-api.ts

# Usage
import { components } from './agenthub-api.ts'
type Agent = components['schemas']['Agent']
```

Using `openapi-generator-cli`:

```bash
# Install
npm install -g @openapitools/openapi-generator-cli

# Generate TypeScript client
openapi-generator-cli generate \
  -i agenthub-openapi.json \
  -g typescript-fetch \
  -o ./agenthub-client

# Usage
import { AgentsApi, Configuration } from './agenthub-client'

const api = new AgentsApi(new Configuration({
  basePath: 'http://localhost:8080',
  accessToken: 'your-token'
}))

const agents = await api.listAgents()
```

### Python

Using `openapi-generator-cli`:

```bash
# Generate Python client
openapi-generator-cli generate \
  -i agenthub-openapi.json \
  -g python \
  -o ./agenthub-python-client

# Install
pip install ./agenthub-python-client

# Usage
from agenthub_client import AgentsApi, Configuration

config = Configuration(
    host="http://localhost:8080",
    access_token="your-token"
)
api = AgentsApi(config)

agents = api.list_agents()
```

Using `httpx` directly:

```python
import httpx
from typing import Optional, Dict, Any

class AgentHubClient:
    def __init__(self, base_url: str, token: str):
        self.base_url = base_url.rstrip('/')
        self.headers = {"Authorization": f"Bearer {token}"}
    
    async def list_agents(self) -> list:
        async with httpx.AsyncClient() as client:
            resp = await client.get(
                f"{self.base_url}/api/agents",
                headers=self.headers
            )
            resp.raise_for_status()
            return resp.json()
    
    async def create_agent(self, name: str, workdir: str, command: str) -> dict:
        async with httpx.AsyncClient() as client:
            resp = await client.post(
                f"{self.base_url}/api/agents",
                headers=self.headers,
                json={
                    "name": name,
                    "workdir": workdir,
                    "command": command,
                    "worktree_mode": "use_existing"
                }
            )
            resp.raise_for_status()
            return resp.json()
    
    async def start_agent(self, agent_id: str) -> None:
        async with httpx.AsyncClient() as client:
            resp = await client.post(
                f"{self.base_url}/api/agents/{agent_id}/start",
                headers=self.headers
            )
            resp.raise_for_status()

# Usage
import asyncio

async def main():
    client = AgentHubClient("http://localhost:8080", "your-token")
    
    # List agents
    agents = await client.list_agents()
    print(f"Found {len(agents)} agents")
    
    # Create and start agent
    agent = await client.create_agent(
        name="api-test",
        workdir="/home/user/project",
        command="echo 'Hello from API'"
    )
    await client.start_agent(agent['id'])

asyncio.run(main())
```

### Go

```go
package main

import (
    "bytes"
    "encoding/json"
    "fmt"
    "net/http"
)

type AgentHubClient struct {
    BaseURL string
    Token   string
}

func (c *AgentHubClient) request(method, path string, body interface{}) (*http.Response, error) {
    var bodyReader *bytes.Reader
    if body != nil {
        bodyBytes, _ := json.Marshal(body)
        bodyReader = bytes.NewReader(bodyBytes)
    } else {
        bodyReader = bytes.NewReader([]byte{})
    }
    
    req, err := http.NewRequest(method, c.BaseURL+path, bodyReader)
    if err != nil {
        return nil, err
    }
    
    req.Header.Set("Authorization", "Bearer "+c.Token)
    req.Header.Set("Content-Type", "application/json")
    
    return http.DefaultClient.Do(req)
}

func (c *AgentHubClient) ListAgents() ([]map[string]interface{}, error) {
    resp, err := c.request("GET", "/api/agents", nil)
    if err != nil {
        return nil, err
    }
    defer resp.Body.Close()
    
    var agents []map[string]interface{}
    err = json.NewDecoder(resp.Body).Decode(&agents)
    return agents, err
}

func main() {
    client := &AgentHubClient{
        BaseURL: "http://localhost:8080",
        Token:   "your-token",
    }
    
    agents, err := client.ListAgents()
    if err != nil {
        panic(err)
    }
    
    fmt.Printf("Found %d agents\n", len(agents))
}
```

### Rust

Using `reqwest`:

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Deserialize)]
struct Agent {
    id: String,
    name: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct CreateAgentRequest {
    name: String,
    workdir: String,
    command: String,
    worktree_mode: String,
}

struct AgentHubClient {
    client: Client,
    base_url: String,
    token: String,
}

impl AgentHubClient {
    fn new(base_url: &str, token: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            token: token.to_string(),
        }
    }
    
    async fn list_agents(&self) -> Result<Vec<Agent>> {
        let resp = self.client
            .get(format!("{}/api/agents", self.base_url))
            .bearer_auth(&self.token)
            .send()
            .await?;
        
        Ok(resp.json().await?)
    }
    
    async fn create_agent(&self, name: &str, workdir: &str, command: &str) -> Result<Agent> {
        let req = CreateAgentRequest {
            name: name.to_string(),
            workdir: workdir.to_string(),
            command: command.to_string(),
            worktree_mode: "use_existing".to_string(),
        };
        
        let resp = self.client
            .post(format!("{}/api/agents", self.base_url))
            .bearer_auth(&self.token)
            .json(&req)
            .send()
            .await?;
        
        Ok(resp.json().await?)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let client = AgentHubClient::new("http://localhost:8080", "your-token");
    
    let agents = client.list_agents().await?;
    println!("Found {} agents", agents.len());
    
    Ok(())
}
```

## Common Automation Patterns

### Pattern 1: CI/CD Integration

Trigger agent runs from CI pipeline:

```yaml
# .github/workflows/agent-run.yml
name: Agent Run

on:
  push:
    branches: [main]

jobs:
  run-agent:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Trigger AgentHub Task
        run: |
          # Login
          TOKEN=$(curl -X POST ${{ secrets.AGENTHUB_URL }}/api/login \
            -H "Content-Type: application/json" \
            -d '{"username":"ci-bot","password":"'"${{ secrets.AGENTHUB_PASSWORD }}"'"}' \
            | jq -r '.token')
          
          # Create agent
          AGENT_ID=$(curl -X POST ${{ secrets.AGENTHUB_URL }}/api/agents \
            -H "Authorization: Bearer $TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
              "name": "ci-'"$GITHUB_RUN_ID"'",
              "workdir": "/github/workspace",
              "command": "run-tests-and-build",
              "worktree_mode": "use_existing"
            }' | jq -r '.id')
          
          # Start agent
          curl -X POST ${{ secrets.AGENTHUB_URL }}/api/agents/$AGENT_ID/start \
            -H "Authorization: Bearer $TOKEN"
          
          # Wait for completion (poll loop)
          while true; do
            STATUS=$(curl -s ${{ secrets.AGENTHUB_URL }}/api/agents/$AGENT_ID \
              -H "Authorization: Bearer $TOKEN" | jq -r '.status')
            echo "Status: $STATUS"
            if [[ "$STATUS" =~ ^(completed|failed|cancelled)$ ]]; then
              break
            fi
            sleep 10
          done
          
          # Check result
          if [ "$STATUS" != "completed" ]; then
            echo "Agent run failed"
            exit 1
          fi
```

### Pattern 2: Scheduled Tasks

```python
#!/usr/bin/env python3
"""Daily maintenance agent runner"""

import schedule
import time
import httpx
import os

AGENTHUB_URL = os.environ["AGENTHUB_URL"]
AGENTHUB_TOKEN = os.environ["AGENTHUB_TOKEN"]

def run_maintenance():
    """Run daily maintenance agent"""
    with httpx.Client() as client:
        # Create maintenance agent
        resp = client.post(
            f"{AGENTHUB_URL}/api/agents",
            headers={"Authorization": f"Bearer {AGENTHUB_TOKEN}"},
            json={
                "name": f"maintenance-{time.strftime('%Y-%m-%d')}",
                "workdir": "/data/maintenance",
                "command": "daily-cleanup-and-report",
                "worktree_mode": "create_worktree",
                "worktree_repo": "/data/repos/maintenance"
            }
        )
        agent = resp.json()
        
        # Start it
        client.post(
            f"{AGENTHUB_URL}/api/agents/{agent['id']}/start",
            headers={"Authorization": f"Bearer {AGENTHUB_TOKEN}"}
        )
        
        print(f"Started maintenance agent: {agent['id']}")

# Schedule daily at 2 AM
schedule.every().day.at("02:00").do(run_maintenance)

while True:
    schedule.run_pending()
    time.sleep(60)
```

### Pattern 3: Team Automation

```python
async def create_and_run_team(client: AgentHubClient, team_spec: dict, input_data: dict):
    """Create team definition and start a run"""
    
    # Create team
    team = await client.post("/api/teams", team_spec)
    team_id = team["id"]
    print(f"Created team: {team_id}")
    
    # Start run
    run = await client.post(f"/api/teams/{team_id}/runs", {
        "input": input_data
    })
    run_id = run["id"]
    print(f"Started run: {run_id}")
    
    # Poll for completion
    while True:
        run = await client.get(f"/api/teams/{team_id}/runs/{run_id}")
        status = run["status"]
        print(f"Run status: {status}")
        
        if status in ["completed", "failed", "cancelled"]:
            break
            
        await asyncio.sleep(10)
    
    # Get results
    steps = await client.get(f"/api/teams/{team_id}/runs/{run_id}/steps")
    return steps

# Usage
team_spec = {
    "name": "feature-implementation",
    "spec": {
        "spec_version": 1,
        "entrypoint": {"type": "task"},
        "members": [
            {"member_id": "coordinator", "description": "Tech lead", "skills": ["planning"]},
            {"member_id": "developer", "description": "Developer", "skills": ["coding"]}
        ],
        "coordinator_member_id": "coordinator"
    }
}

results = await create_and_run_team(client, team_spec, {"goal": "Implement feature X"})
```

### Pattern 4: Actor Mailbox Automation

```python
async def send_command_to_team(client: AgentHubClient, run_id: str, command: str):
    """Send a command to a running team via actor mailbox"""
    
    # Get team members
    members = await client.get(f"/api/teams/runs/{run_id}/members")
    coordinator_id = members["coordinator"]["actor_id"]
    
    # Send message to coordinator
    await client.post("/api/actor/mailbox/send", {
        "from_actor_id": "automation-bot",
        "to_actor_id": coordinator_id,
        "run_id": run_id,
        "payload": {
            "type": "command",
            "text": command
        }
    })

async def check_team_messages(client: AgentHubClient, actor_id: str, run_id: str):
    """Check inbox for team responses"""
    
    messages = await client.get(
        f"/api/actor/{actor_id}/mailbox",
        params={"run_id": run_id, "limit": 10}
    )
    
    for msg in messages:
        print(f"From {msg['from_actor_id']}: {msg['payload']}")
        # Acknowledge receipt
        await client.post(f"/api/actor/mailbox/{msg['id']}/ack")
```

## Testing and Contract Verification

### API Contract Tests

```python
# test_agenthub_contract.py
import pytest
import httpx
from jsonschema import validate

class TestAgentHubContract:
    @pytest.fixture
    def client(self):
        return httpx.Client(
            base_url="http://localhost:8080",
            headers={"Authorization": "Bearer test-token"}
        )
    
    def test_list_agents_returns_valid_schema(self, client):
        """Verify /api/agents returns expected schema"""
        resp = client.get("/api/agents")
        assert resp.status_code == 200
        
        agents = resp.json()
        assert isinstance(agents, list)
        
        # Validate schema
        agent_schema = {
            "type": "object",
            "required": ["id", "name", "status"],
            "properties": {
                "id": {"type": "string"},
                "name": {"type": "string"},
                "status": {"type": "string", "enum": ["created", "running", "completed", "failed"]}
            }
        }
        
        for agent in agents:
            validate(instance=agent, schema=agent_schema)
    
    def test_create_agent_requires_name(self, client):
        """Verify validation error for missing name"""
        resp = client.post("/api/agents", json={"workdir": "/tmp"})
        assert resp.status_code == 400
        assert "name" in resp.json()["error"].lower()
```

### Spec Version Pinning

```bash
# Pin spec version in CI
SPEC_VERSION=$(curl -s http://localhost:8080/api/openapi.json | jq -r '.info.version')
echo "Testing against API spec version: $SPEC_VERSION"

# Store in artifacts for traceability
echo "$SPEC_VERSION" > .spec-version
```

## Reliability Best Practices

### 1. Idempotency

Design automation to be idempotent:

```python
async def ensure_agent_exists(client, name: str, config: dict) -> str:
    """Create agent if it doesn't exist, return ID"""
    
    # Check if agent exists
    agents = await client.get("/api/agents")
    for agent in agents:
        if agent["name"] == name:
            print(f"Agent {name} already exists: {agent['id']}")
            return agent["id"]
    
    # Create new
    agent = await client.post("/api/agents", {**config, "name": name})
    print(f"Created agent {name}: {agent['id']}")
    return agent["id"]
```

### 2. Exponential Backoff

```python
import random

async def retry_with_backoff(func, max_retries=5):
    """Retry with exponential backoff"""
    for attempt in range(max_retries):
        try:
            return await func()
        except httpx.HTTPStatusError as e:
            if e.response.status_code in [429, 502, 503, 504]:
                delay = (2 ** attempt) + random.uniform(0, 1)
                print(f"Retry {attempt + 1}/{max_retries} after {delay:.1f}s")
                await asyncio.sleep(delay)
            else:
                raise
    raise Exception("Max retries exceeded")
```

### 3. Pagination Handling

```python
async def list_all_agents(client) -> list:
    """List all agents handling pagination"""
    all_agents = []
    cursor = None
    
    while True:
        params = {"limit": 100}
        if cursor:
            params["cursor"] = cursor
            
        resp = await client.get("/api/agents", params=params)
        data = resp.json()
        
        all_agents.extend(data["items"])
        
        cursor = data.get("next_cursor")
        if not cursor:
            break
    
    return all_agents
```

### 4. Token Refresh

```python
class AgentHubClient:
    def __init__(self, base_url: str, username: str, password: str):
        self.base_url = base_url
        self.username = username
        self.password = password
        self.token = None
        self.token_expires = 0
    
    async def ensure_token(self):
        """Refresh token if expired"""
        if time.time() < self.token_expires - 60:  # 60s buffer
            return
            
        async with httpx.AsyncClient() as client:
            resp = await client.post(
                f"{self.base_url}/api/login",
                json={"username": self.username, "password": self.password}
            )
            data = resp.json()
            self.token = data["token"]
            self.token_expires = time.time() + data.get("expires_in", 3600)
```

## Monitoring Integration

### Prometheus Metrics Example

```python
from prometheus_client import Counter, Histogram

api_calls_total = Counter(
    'agenthub_api_calls_total',
    'Total API calls',
    ['method', 'endpoint', 'status']
)

api_latency = Histogram(
    'agenthub_api_latency_seconds',
    'API call latency',
    ['method', 'endpoint']
)

async def instrumented_request(client, method: str, path: str, **kwargs):
    """Make request with metrics"""
    start = time.time()
    
    try:
        resp = await client.request(method, path, **kwargs)
        status = resp.status_code
        return resp
    except Exception as e:
        status = "error"
        raise
    finally:
        duration = time.time() - start
        api_calls_total.labels(method=method, endpoint=path, status=status).inc()
        api_latency.labels(method=method, endpoint=path).observe(duration)
```

## Security Considerations

1. **Token Storage**: Store tokens in secrets management (not in code)
2. **HTTPS Only**: Use TLS in production
3. **Token Rotation**: Implement automatic token refresh
4. **Least Privilege**: Use dedicated service accounts
5. **Audit Logging**: Log all API actions for compliance

## Related Pages

- [Team Workbench](./team-workbench.md)
- [Deployment Overview and Topology](../deployment/overview-and-topology.md)
- [API Error Reference](../operations/api-error-reference.md)
