# Independent custody client receiver

Status: Feature migration delivered; native consumer checks pending.

Woodshed, Hocket and the active relationship-recipe fixture explicitly select
`graphshell-endpoint/custody-client`. Their existing custody imports remain
valid; the endpoint default feature set is now empty. The root, Hocket and
recipe locks receive published Mere `c6290142866eefead385e728c5c5da24ac6f624a`.
Historical receipt copies and unrelated nested ports are unchanged.

The existing Eidetic owners remain distinct. Root and Hocket retain aa94;
the recipe retains its direct 341 source, while current path dependencies use
aa94 and the new Mere supplier brings 01ee. aa94 and 01ee are divergent Git
histories, so this migration does not assert they are interchangeable. Six
root and two Hocket dependency strings were qualified to preserve their exact
old targets after the incoming graph introduced duplicate package names.
Cargo subsequently normalized the locks to the current feature graph.

Lock review includes resolved edges. Unrelated registry target versions and
NetRender selections remain unchanged. Hocket's Tokio macros edge is activated
by the new feature closure. The recipe also gains optional ObjC/Metal edges;
its stale path-package entries now reflect current local manifests (including
muniment 0.2 and persona-picker). These are not runtime qualification of the
storage or presentation boundaries.

Focused receiving commands are:

- root: `cargo check --locked -p woodshed-genet --tests`;
- `ports/hocket`: `cargo check --locked -p hocket-genet --tests`;
- `validation/relationship-recipe`: `cargo test --locked real_domain_disclosure_compiles_and_has_routed_occurrence_endpoints`.

These had not executed when this migration was committed: the preceding
Cleromancy dependency build reached the bounded multi-receiver time limit.
The native custody tests require isolated keeper fixtures and remain ignored;
compiling them does not qualify live custody or restart behavior. Windows and
GUI execution are not qualified.

Resolved-edge reports are retained on T-PC under
`/home/markik/Code/testing/custody-projection-split-20261029` as
`woodshed-lock-normalized.json`, `hocket-lock-normalized.json` and
`recipe-lock-normalized.json`. The shared runner uses 3 GiB memory, zero swap,
150 percent CPU and one build job; further check results are recorded separately.
