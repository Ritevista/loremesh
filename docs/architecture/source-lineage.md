# Source and processing lineage

Source lineage answers “which authoritative bytes support this?” It follows finding → claim → evidence range → artifact → immutable snapshot → source. A digest mismatch invalidates the snapshot rather than silently changing its meaning.

Processing lineage answers “what actions produced this derived object?” It uses trace processing nodes and origin-labelled edges. Manual creation is recorded as `Manual`; deterministic transformations as `Deterministic`; parsers as `Extracted`; probabilistic reasoning as `Inferred`; external preserved relationships as `Imported`.

The two lineages may meet at evidence but are not interchangeable. A processing record cannot substitute for evidence, and a source reference does not claim how a result was produced. Trace path validation rejects missing nodes, cycles, and disconnected claimed paths.

```d2
direction: right

source-lineage: Source lineage {
  finding -> claim -> evidence-range -> artifact -> immutable-snapshot -> source
}

processing-lineage: Processing lineage {
  derived-object -> processing-step: produced by
  processing-step -> evidence-range: consumed
}

source-lineage.evidence-range -> processing-lineage.evidence-range: same evidence identity {
  style.stroke-dash: 4
}
```

The dashed join is identity, not substitution: processing lineage explains production, while source lineage anchors a claim to authoritative bytes.
