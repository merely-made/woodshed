# W3C Web Annotation test suite, vendored

The JSON Schema definitions and the MUST-level assertions of the W3C Web
Annotation test suite, as `tests/w3c_conformance.rs` runs them over Redshank's
export.

- Source: <https://github.com/w3c/web-annotation-tests>, commit
  `adedd9a5f06d9daa75bbfbb97aa9215cd09b4bb5` (2019-03-05, the latest on
  `master` when vendored on 2026-09-26).
- `definitions/`: every file from the repository's `definitions/` directory,
  unchanged.
- `annotations/annotationMusts.test`: the suite's list of MUST assertions,
  unchanged. Each assertion it names is copied to the same relative path,
  unchanged. Optional (MAY/SHOULD) assertions are not vendored.
- `LICENSE.md`: the repository's licence file. The tests are dual-licensed
  under the W3C Test Suite License and the 3-Clause BSD License; see
  <http://www.w3.org/Consortium/Legal/2008/04-testsuite-copyright.html>.

The schemas are JSON Schema draft-04 and reference one another by bare file
name (`id.json#/definitions/stringUri`); the test resolves those names against
`definitions/` regardless of the referring file's directory.
