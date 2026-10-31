# To-do

## Pending verification
- Visually check the responsive grid in very wide and very narrow windows. If
  the column split doesn't work in some case, fall back to `FlexRow` with
  `SpaceBetween` and note it here.
- Check bar cropping with new covers that have light uniform backgrounds.
- Confirm on screen that both `S29E03` variants show under the same episode
  with their label.

## Optimization
- Investigate why the 4 threads only improved the first scan by 32%.

## Later
- Button to wipe the whole catalog (database and cache) with a confirmation,
  to test the first scan; the collection folder is never touched.
- First import (~26 s with an empty database, 292 tracks and 208 covers):
  show progress and fill the list as it goes, instead of waiting until the end.
- Built-in viewers/players: will not be implemented; opening stays external.
- Review old v1 thumbnails (16:9) left unused in the cache.

## Unverified
- Read `cover.jpg` and hash it once per directory, not once per track (unverified).
- Detect `cover.jpg` changes even when the audio file's size and date don't change (unverified).
- Group by the album artist (TPE2) instead of the track's first artist (unverified).
- Retry tag reads that ended in error, without waiting for the file to change (unverified).
- Reduce `tokio` features to the ones actually used (unverified).
- Movie posters: read an image with the same name as the video next to it
  (optional and local); series would need a per-series folder; today every
  video uses an icon (unverified).
- With 8 threads, two threads may decode the same cover at once (race);
  in-flight dedup by hash is missing (unverified).
- Try a rescaling filter faster than Lanczos3, or smaller thumbnails (e.g.
  384 px), to cut render/JPEG cost; compare visual quality first (unverified).
- Details panel with a very narrow and tall window: check whether it docks
  wrong or covers content (unverified).
