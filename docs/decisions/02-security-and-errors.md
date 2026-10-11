# 11/10/2026

## ADR 02: Security and Errors

### 1 **Errors in main**
- For now we keep error escalation simple to only return the cause chain information of an error, will be updated later to return a different error integer when more relevant in [S6 or S14](../ROADMAP.md#v1-a-jarvis-lite-youll-actually-use-daily).

### 2 **RawConfig and Config**
- This is just about validation of the data and i've decided to handle it into two classes, one which handles the form of the data and the other which `resolves` (checks the information itself) that is used inside the code. `Config` is considered clean, otherwise an error of the relevant kind is returned.

### 3 **Error on missing config**
- This is to follow the default closed when missing philosophy of the project, since this is trying to be built around the fact that it will act autonomously we must make sure that the user knows what will happen, and this goes through having the user configure and have a configuration file for the relevant crates. The goal will be to have a restricted default, and make modification easy later on. 
- The file must exist, though each setting in it is either required (the workspaces) or will have a restrictive default (toolcalls limits)
