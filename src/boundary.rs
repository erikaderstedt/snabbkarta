
use super::dtm::{DigitalTerrainModel};
use delaunator::EMPTY;

struct Boundary <'a> {
    halfedges: Vec<usize>,
    index: usize,
    dtm: &'a DigitalTerrainModel,
}

impl Boundary <'_> {

    fn add_halfedge(&mut self, halfedge: usize, recurse: &dyn FnOnce(&Self, usize) -> bool) {
        let opposite = self.dtm.halfedges[halfedge];
        let triangle = opposite / 3;
        if opposite == EMPTY {}
    }
}
