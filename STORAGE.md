- package tarballs storage is pretty inefficient
    - whole new copy for each version
    - what if new versions turned old versions into patches?


okay here's the thought:

1. assume a delta compression strategy makes it cheaper to COPY bytes from a source than to INSERT new bytes
2. given 1, that means it is cheaper to model diffs as DELETION rather than INSERTION
3. assume files and codebases tend to grow over time
4. what if, every time we published a new version of a package, we turned the last (unzipped) latest version
   into a diff stream against the newest version and stored it

implications thus far:

- package publishes have to calculate a diff from last version to new version and replace the old version
- package deletions (not yanks!) have to restore the old version
- beta tags should probably be left out of this
- might make it cheaper to calculate relative sizes
- requests for old versions have to apply diffs on the fly (which might be slower)
- we MIGHT want to enforce a given sort on tarballs as they come in
    - makes writes more expensive
    - but you could decouple this into a "receive package"/"optimize diffs"
      pass on the server side -- serve the tarballs until the optimize task
      completes, at which point you're storing and serving less data

BUT:

1. what if, when the client sent a header like "have-versions: base64(major judy array).base64(minor judy array).base64(patch judy)",
   we determined the shortest distance to a patch and responded with a base, partial patch + checksum?
2. advanced installations could calculate all of the patch diffs and store them, with the thought that one-time compute costs are
   cheaper than repeated bandwidth costs

judy arrays:

https://nothings.org/computer/judy/#size

---

extensions:

you could picture a version of this where we add a `want-versions` flag in the future, which would reduce roundtrips
