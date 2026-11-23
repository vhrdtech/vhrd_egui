//! ve_gpu's tests: the shaders against WebGL2, batches rendered headless and checked by pixel, and snapshots of
//! each primitive in both themes. After an intended visual change:
//! `UPDATE_SNAPSHOTS=1 cargo test -p ve_gpu --test it` and commit the images under `tests/snapshots/`.

mod render;
mod scenes;
mod shaders;
mod snapshots;
