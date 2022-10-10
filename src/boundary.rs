
use super::dtm::{DigitalTerrainModel,Halfedge,TriangleWalk};
use crate::geometry::Point3D;
use delaunator::EMPTY;
use super::Sweref;


pub trait Boundary {
    fn claim(&mut self, triangle: usize);
    fn push_halfedge(&mut self, h: Halfedge);
    fn dtm(&self) -> &DigitalTerrainModel;
    fn get_halfedges(&self) -> &Vec<Halfedge>;

    fn should_recurse(&self, halfedge: Halfedge) -> bool;

    fn add_halfedge(&mut self, halfedge: Halfedge) {
        assert_ne!(halfedge, EMPTY, "Halfedge is empty!");
        let opposite = self.dtm().opposite(halfedge);
        let triangle = opposite / 3;
        if opposite != EMPTY && self.should_recurse(opposite) {
            self.claim(triangle);
            self.add_halfedge(opposite.next());
            self.add_halfedge(opposite.prev());
        } else {
            // Just add the half-edge.
            self.push_halfedge(halfedge);
        }
    }
    
    fn grow_from_seed(&mut self, triangle: usize) {
        self.claim(triangle);
        self.add_halfedge(triangle*3 + 0);
        self.add_halfedge(triangle*3 + 1);
        self.add_halfedge(triangle*3 + 2);
    }

    fn split_into_outer_edge_and_islands(&self) -> (Vec<Halfedge>, Vec<Vec<Halfedge>>) {
        let mut remaining = vec![self.get_halfedges().clone()];
        let mut finished: Vec<Vec<Halfedge>> = Vec::new();
    
        while remaining.len() > 0 {
            let s = split_loop_as_needed(self.dtm(), remaining.pop().unwrap());
            if let Some(o) = s.1 {
                if o.len() > 0 { 
                    remaining.push(o); 
                } 
                remaining.push(s.0);
            } else {
                finished.push(s.0);
            }
        }
    
        // One is clockwise, the rest counter-clockwise.
        let lake_and_islands: (Vec<Vec<Halfedge>>,Vec<Vec<Halfedge>>) 
            = finished.into_iter().partition(|a| is_clockwise(self.dtm(), a));
        assert_eq!(lake_and_islands.0.len(), 1, "Not exactly one clockwise fragment");
    
        (lake_and_islands.0.into_iter().next().unwrap(), lake_and_islands.1)
    }
    
}

fn is_clockwise(dtm: &DigitalTerrainModel, halfedges: &Vec<Halfedge>) -> bool {
    let num_halfedges = halfedges.len();
    let vertices: Vec<&Point3D> = halfedges.iter()
        .cycle()
        .take(num_halfedges+1)
        .map(|h| &dtm.points[dtm.vertices[*h]])
        .collect();

    vertices[..]
        .windows(2)
        .map(|p| p[0].x*p[1].y - p[0].y * p[1].x)
        .sum::<f64>() < 0f64
}

fn split_loop_as_needed(dtm: &DigitalTerrainModel, halfedges: Vec<Halfedge>) -> (Vec<Halfedge>, Option<Vec<Halfedge>>) {
    let original_length = halfedges.len();

    for (i, h) in halfedges.iter().enumerate() {
        let opposite = dtm.opposite(*h);
        if opposite == EMPTY { continue };

        if let Some(steps_from_i_to_opposite) = halfedges.iter()
            .skip(i)
            .take(original_length)
            .position(|m| *m == opposite) {
            let opposite_located_at = i + steps_from_i_to_opposite;
            let forward = halfedges.iter().skip(i);
            let backward = halfedges.iter().take(opposite_located_at+1).rev().map(|h| dtm.opposite(*h));
            let bridge_length = forward.zip(backward).take_while(|m| *m.0 == m.1).count();
            // Two different cases to handle:
            // 1) An appendix: just one or more dangling points, no island on the end.
            // 2) An actual island.
            // In the appendix case, bridge_length = opposite + 1 - i, because the iterator "wraps around" at the tip of
            // the appendix.
            // In the island case bridge_length*2 + island_length = opposite + 1 - i
            // NOTE: If i = 0, then we can't be sure that the full length of the bridge has been found. 
            if bridge_length == steps_from_i_to_opposite + 1 {
                return (halfedges.into_iter().cycle().skip(opposite_located_at+1).take(original_length - bridge_length).collect(), Some(Vec::new()));
            } else {
                let island_length = steps_from_i_to_opposite + 1 - bridge_length*2;
                let remaining_length = original_length - (steps_from_i_to_opposite + 1);
                return (halfedges.clone().into_iter().cycle().skip(opposite_located_at+1).take(remaining_length).collect(),
                    Some(halfedges.into_iter().skip(i + bridge_length).take(island_length).collect()));
            }
        }
    }
    (halfedges, None)
}


pub fn extract_vertices(dtm: &DigitalTerrainModel, halfedges: &Vec<Halfedge>, islands: &Vec<Vec<Halfedge>>) -> Vec<Vec<Sweref>> {
    let mut pts: Vec<Vec<Sweref>> = Vec::new();
    let halfedge_to_sweref = |h: &usize| -> Sweref {
        let p = dtm.points[dtm.vertices[*h]];
        Sweref { east: p.x, north: p.y, }
    };
    pts.push(halfedges.iter().map(halfedge_to_sweref).collect());
    for island in islands.iter() {
        pts.push(island.iter().map(halfedge_to_sweref).collect());
    }
    pts
}

pub fn extract_interior_segments(dtm: &DigitalTerrainModel, halfedges: &Vec<Halfedge>) -> Vec<Vec<Sweref>> {
    let mut segs: Vec<Vec<Sweref>> = Vec::new();
    let mut cur: Vec<Sweref> = Vec::new();
    let halfedge_to_sweref = |h: &usize| -> Sweref {
        let p = dtm.points[dtm.vertices[*h]];
        Sweref { east: p.x, north: p.y, }
    };
    for h in halfedges.iter() {
        let triangle = h / 3;
        if dtm.exterior[triangle] && cur.len() > 0 {
            segs.push(cur);
            cur = Vec::new();
        }
        if !dtm.exterior[triangle] {
            cur.push(halfedge_to_sweref(h));
        }
    }
    if cur.len() > 0 { segs.push(cur); }
    segs
}

