# Team Mailbox Phase 3 Transfer Slice

## Summary

This slice adds one explicit mailbox transfer path for reply-required human-originated work.

- a claimed or pending reply-required mailbox item can now be transferred from one team member to
  another canonical team member without pretending the work is completed;
- the original mailbox item is released with `mailbox_resolution.kind = transferred`;
- the reply obligation is reissued as a new pending mailbox message for the target actor;
- reply-obligation summaries treat `released + transferred` as terminal for the source item.

## Scope

This change does not close Team mailbox phase 3.

It only covers explicit actor-to-actor transfer for reply-required mailbox work. Remaining phase 3
items still include broader inbound-envelope normalization, wider `requires_user_visible_reply`
coverage, and distinct cross-actor takeover semantics.

## Validation

- focused API regression for transfer reassigning the open reply obligation to the target actor
- existing escalation and triage reply-obligation tests remain the adjacent coverage surface
