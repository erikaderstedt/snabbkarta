
use super::dtm::{DigitalTerrainModel,Halfedge,TriangleWalk,Point3D};
use delaunator::EMPTY;
use std::collections::VecDeque;

pub struct Boundary<'a> {
    pub halfedges: Vec<Halfedge>,
    pub index: usize,

    // Shared between all boundary objects:
    pub dtm: &'a DigitalTerrainModel,
    pub indices_for_each_triangle: &'a mut Vec<usize>,
}

impl<'a> Boundary<'a> {

    pub fn grow_from_triangle(&mut self, triangle: usize, recurse: &dyn Fn(&Self, usize) -> bool) {
        self.indices_for_each_triangle[triangle] = self.index;
        self.add_halfedge(triangle*3, recurse);
        self.add_halfedge(triangle*3+1, recurse);
        self.add_halfedge(triangle*3+2, recurse);
    }

    // Adds a halfedge to the end of a closed loop being constructed and potentially also recurses to add the other edges of the triangle on the other side of the halfedge.
    fn add_halfedge(&mut self, halfedge: Halfedge, recurse: &dyn Fn(&Self, usize) -> bool) {
        let opposite = self.dtm.opposite(halfedge);
        let triangle = opposite / 3;
        if opposite == EMPTY && recurse(self, opposite) {
            self.indices_for_each_triangle[triangle] = self.index; // Claim this triangle.
            self.add_halfedge(opposite.next(), recurse);
            self.add_halfedge(opposite.prev(), recurse);
        } else {
            // Just add the half-edge.
            self.halfedges.push(halfedge);
        }
    }

    fn split_loop_as_needed(dtm: &DigitalTerrainModel, halfedges: Vec<Halfedge>) -> (Vec<Halfedge>, Option<Vec<Halfedge>>) {
        let original_length = halfedges.len();

        for (i, h) in halfedges.iter().enumerate() {
            let opposite = dtm.opposite(*h);
            if opposite == EMPTY { continue };

            if let Some(opposite_located_at) = halfedges.iter().skip(i).position(|m| *m == opposite) {
                let forward = halfedges.iter().skip(i);
                let backward = halfedges.iter().take(opposite_located_at+1).rev();
                let bridge_length = forward.zip(backward).take_while(|m| *m.0 == *m.1).count();
                // Two different cases to handle:
                // 1) An appendix: just one or more dangling points, no island on the end.
                // 2) An actual island.
                // In the appendix case, bridge_length = opposite + 1 - i, because the iterator "wraps around" at the tip of
                // the appendix.
                // In the island case bridge_length*2 + island_length = opposite + 1 - i
                // NOTE: If i = 0, then we can't be sure that the full length of the bridge has been found. 
                if bridge_length == opposite_located_at + 1 - i {
                    return (halfedges.into_iter().cycle().skip(i).take(original_length - bridge_length).collect(), None);
                } else {
                    let island_length = opposite_located_at + 1 - i - bridge_length*2;
                    let remaining_length = original_length - (opposite + 1 - i);
                    return (halfedges.clone().into_iter().cycle().skip(opposite_located_at).take(remaining_length).collect(),
                        Some(halfedges.into_iter().skip(i + bridge_length).take(island_length).collect()));
                }
            }

        }
        (halfedges, None)
    }

}
