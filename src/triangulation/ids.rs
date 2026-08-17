/// An opaque handle to one of a [`super::Triangulation2`]'s vertices.
///
/// Valid only for the `Triangulation2` that produced it — comparing or
/// indexing with a handle from a different triangulation is not checked
/// and will silently look up the wrong element (same convention as
/// indexing a `Vec` with an index from a different `Vec`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VertexId(pub(super) u32);

/// An opaque handle to one of a [`super::Triangulation2`]'s undirected
/// edges. See [`VertexId`]'s doc comment for the cross-triangulation
/// caveat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeId(pub(super) u32);

/// An opaque handle to one of a [`super::Triangulation2`]'s triangular
/// faces. See [`VertexId`]'s doc comment for the cross-triangulation
/// caveat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FaceId(pub(super) u32);

impl VertexId {
    pub(super) fn raw(self) -> u32 {
        self.0
    }
}

impl FaceId {
    pub(super) fn new(raw: u32) -> Self {
        FaceId(raw)
    }

    pub(super) fn raw(self) -> u32 {
        self.0
    }
}
