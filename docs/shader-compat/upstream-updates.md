# Explicit upstream updates

The baseline remains `42d1e6d7d77ed300fcc4d644ce1444c25356085c`.
These isolated official MIT-licensed changes were reviewed and cherry-picked
with original authorship, rather than merging their intervening main history:

- `9b0a0d56a9e503366da50f5ca83d8a123dad47d2`: #12746 Auto HDR, system display
  reporting, output availability refresh and its frame-policy tests.
- `00299919d92b965740ac12667b3e3a2cdb67b176`: #12746 translation-key correction.
- `40d0364accbe75707ca41ed5592a854fa6d88bcd`: #12835's isolated frame-outcome and
  refusal-fingerprint change, still open at inspection. Its prerequisites are the
  two commits above; its merge-main ancestor was not imported.

This chain does not implement native API 0.2 HDR output, linear blending,
transactional hot reload or split-guest frame following. The existing
`HORIZON_HDR_FRAME=float` gate remains. Physical display HDR is a separate check.
The compatibility fork must verify its own GLSL handoff and refusal paths before
claiming HDR support for any pack.

Reproduce the source review with `git show <revision>` and compare the relevant
PRs at `https://github.com/horizon-rust/horizon/pull/12746` and `/pull/12835`.
