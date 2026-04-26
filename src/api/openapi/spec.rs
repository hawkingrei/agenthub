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
            "required": ["leader_prompt", "worker_prompt"],
            "properties": {
              "leader_prompt": { "type": "string" },
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
              "role": { "type": "string", "enum": ["leader", "worker"] },
              "model": { "type": ["string", "null"] },
              "prompt": { "type": ["string", "null"] },
              "skills": { "type": "array", "items": { "type": "string" } },
              "pending_inbox_count": { "type": "integer", "format": "int64" },
              "status": { "type": "string" },
              "latest_step": {
                "allOf": [{ "$ref": "#/components/schemas/TeamStepRecord" }],
                "nullable": true
              },
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
              "leader_member_id": { "type": ["string", "null"] },
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
                "description": "Default leader and worker prompts",
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
                      "type": "object",
                      "required": ["messages", "pending_count"],
                      "properties": {
                        "messages": {
                          "type": "array",
                          "items": { "$ref": "#/components/schemas/TeamActorMessageRecord" }
                        },
                        "next_cursor": {
                          "type": ["integer", "null"],
                          "format": "int64"
                        },
                        "pending_count": {
                          "type": "integer",
                          "format": "int64"
                        }
                      }
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
