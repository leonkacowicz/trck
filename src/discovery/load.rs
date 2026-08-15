//! Opening a tracker, and where its files are when it has any.
//!
//! Split from [`super::content`] because loading and reading are different moments: this
//! runs once per invocation and decides whether there is a tracker at all, while the
//! content accessors run repeatedly once that is settled.

use super::content::{INDEX_NAME, SUMMARY_NAME};
use super::{CONFIG_NAME, Ctx, ITEMS_DIR, Source, check_layout};
use crate::config::Config;
use std::path::{Path, PathBuf};

impl Ctx {
    /// Load the tracker at `dir`, applying the format guard.
    ///
    /// The guard lives here because every verb builds a `Ctx`, so there is no call site
    /// left to forget it. `guard_format = false` is for `update`: it is the remedy the
    /// refusal names, so guarding it would leave no way to get an engine that
    /// understands the tracker.
    pub(crate) fn load(source: Source, guard_format: bool) -> Result<Ctx, String> {
        let (text, label) = match &source {
            // A missing config reads as defaults rather than an error: discovery already
            // guaranteed the file is there, and a half-made tracker should not panic.
            Source::Dir(dir) => {
                let path = dir.join(CONFIG_NAME);
                (std::fs::read_to_string(&path).unwrap_or_default(), path.display().to_string())
            },
            // A ref is different. Discovery guaranteed only that the revision *resolves*, so
            // a tree with no config is a revision that is not a tracker — and saying that
            // beats reading it as an empty one and reporting a tracker with no issues.
            Source::Ref { rev, cwd } => {
                let text = crate::git::show(cwd, rev, CONFIG_NAME)?.ok_or_else(|| format!("git ref '{rev}' holds no {CONFIG_NAME}, so it is not a tracker"))?;
                (text, format!("{rev}:{CONFIG_NAME}"))
            },
        };
        let config = Config::parse(&text, &label)?;
        if guard_format && let Some(msg) = config.check_format() {
            return Err(msg);
        }
        // Only a directory can be in the pre-0.23 layout: per-status folders were a
        // filesystem arrangement, and a ref-backed tracker never had one to migrate.
        if guard_format
            && let Source::Dir(dir) = &source
            && let Some(msg) = check_layout(dir)
        {
            return Err(msg);
        }
        Ok(Ctx { source, config })
    }

    /// The tracker on disk, or a refusal naming the ref it is on instead.
    ///
    /// Every caller is something that has to touch files — applying a changeset,
    /// installing a hook, writing `.gitattributes`. Those arrive for a ref-backed tracker
    /// in the writes tranche; until then the refusal is the honest answer, and it names the
    /// ref so the reader knows the tracker was *found* rather than missing.
    pub(crate) fn dir(&self) -> Result<&Path, String> {
        match &self.source {
            Source::Dir(dir) => Ok(dir),
            Source::Ref { rev, .. } => Err(format!("the tracker is git ref '{rev}', which has no files on disk")),
        }
    }

    // The path accessors are the write side's, and they are fallible for the same reason
    // [`Ctx::dir`] is. An infallible version falling back to a relative path looks harmless
    // and is not: it made `trck path` print `items/<id>-<slug>.md` for a ref-backed
    // tracker — which reads as real, resolves against whatever directory the caller happens
    // to be in, and is not there.

    pub(crate) fn index_path(&self) -> Result<PathBuf, String> {
        Ok(self.dir()?.join(INDEX_NAME))
    }

    pub(crate) fn items_dir(&self) -> Result<PathBuf, String> {
        Ok(self.dir()?.join(ITEMS_DIR))
    }

    pub(crate) fn summary_path(&self) -> Result<PathBuf, String> {
        Ok(self.dir()?.join(SUMMARY_NAME))
    }
}
