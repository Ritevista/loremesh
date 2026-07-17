# System context

An engineer invokes LoreMesh against a local workspace. LoreMesh reads explicitly selected local files, stores immutable snapshot bytes below `.loremesh/objects`, records metadata in `.loremesh/loremesh.db`, renders terminal views, and writes requested exports. It makes no network calls.

Future documentation, issue, Git, CI, graph, and model systems are external. Their adapters must disclose network/process activity, translate vendor models into canonical types, observe time and output limits, and never become required for core workflows.

Trust boundaries exist at imported bytes, workspace paths, export destinations, terminal output, SQLite data, and future subprocess messages. Local filesystem access does not imply permission to upload content.
