use crate::rt::VersionVec;

#[derive(Debug, Clone)]
pub(crate) struct Access {
    path_id: usize,
    dpor_vv: VersionVec,
}

impl Access {
    pub(crate) fn new(path_id: usize, version: &VersionVec) -> Access {
        Access {
            path_id,
            dpor_vv: version.clone(),
        }
    }

    pub(crate) fn set(&mut self, path_id: usize, version: &VersionVec) {
        self.path_id = path_id;
        self.dpor_vv.set(version);
    }

    /// Location in the path
    pub(crate) fn path_id(&self) -> usize {
        self.path_id
    }

    pub(crate) fn version(&self) -> &VersionVec {
        &self.dpor_vv
    }

    pub(crate) fn happens_before(&self, version: &VersionVec) -> bool {
        self.dpor_vv <= *version
    }
}
