# Dependency decision policy

A major dependency is accepted only for an approved requirement or a measured
bottleneck. Popularity alone is not evidence. Add the decision beside the
change using this template:

```text
Requirement:
Owner and review date:
Crate/version/features:
Maintenance evidence (release cadence, bus factor, advisories):
License and NOTICE impact:
Locked transitive dependency delta:
Clean build-time delta:
Universal/release binary-size delta:
Runtime or quality measurement and acceptance threshold:
Alternatives tested, including the standard library/current stack:
Sandbox, unsafe, network and parser attack-surface change:
Exit strategy and data/protocol compatibility:
Decision: accept / experiment-only / reject
```

Production dependencies use minimum features, a committed lockfile, `cargo
deny`, artifact SBOM/audit and quarterly stale/advisory review. Experiment-only
dependencies remain optional and must not leak into the default graph. Removal
must preserve persisted scene/preferences compatibility or include a migration.
