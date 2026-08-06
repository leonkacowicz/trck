# release: the artifact carried its own staging tree, and publish choked on it

## Summary
The first run of `release.yml` (v0.26.0) built all six targets, installed the musl artifact
and passed 227/227 conformance against it — and then failed at the last step:

```
read dist/trck-v0.26.0-aarch64-apple-darwin: is a directory
```

The packaging steps staged the tarball's contents in `dist/<name>/` and then uploaded
`dist/*.*`. That glob was meant to say "the files", but `<name>` is
`trck-v0.26.0-aarch64-apple-darwin` — the version number puts dots in the *directory* name,
so the glob matched it too and every artifact shipped a loose copy of its own contents
beside the tarball. `gh release upload dist/*` then tried to upload a directory.

Two consequences, one worse than the other.

The visible one: the release was created and no assets were attached. Since `install.sh`
resolves `/releases/latest`, the install path was broken for as long as that release stood
as the newest — it would resolve a tag whose assets do not exist.

The quiet one: `verify` installed the binary with `find dist -name trck -type f`, and the
stray staging copy sat right next to the tarball. The gate that exists to prove *the
published artifact* passes its own spec could have been satisfied by a file the tarball
never contained. It happened to be the same binary here. It did not have to be.

## Acceptance criteria
- [x] Staging happens outside `dist/`, so `dist/` holds exactly the publishable files.
- [x] A step fails the build if anything in `dist/` is a directory, rather than leaving the
      next glob to discover it.
- [x] `verify` unpacks into its own directory and installs the binary by an exact path, so
      it cannot certify a file that was not inside the tarball.
- [x] Observed end to end on a real tag: v0.26.0 re-cut on the fix — six targets, twelve
      assets, checksums present, and `install.sh` verified the checksum on the way in.
