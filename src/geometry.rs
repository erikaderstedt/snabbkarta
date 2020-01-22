use super::sweref_to_wgs84::Sweref;
use std::cmp::Ordering;

pub struct LineSegment {
    pub p0: Sweref,
    pub p1: Sweref,
}

impl LineSegment {

    pub fn create(a: &Sweref, b: &Sweref) -> LineSegment {
        LineSegment { p0: a.clone(), p1: b.clone() }
    }

    pub fn segments_from_bounding_box(sw: &Sweref, ne: &Sweref) -> Vec<LineSegment> {
        let nw = Sweref { north: ne.north, east: sw.east };
        let se = Sweref { north: sw.north, east: ne.east };
        vec![
            LineSegment { p0: sw.clone(), p1: se.clone() },
            LineSegment { p0: se.clone(), p1: ne.clone() },
            LineSegment { p0: ne.clone(), p1: nw.clone() },
            LineSegment { p0: nw.clone(), p1: sw.clone() },
        ]
    }

    pub fn intersection_with(&self, other: &LineSegment) -> Option<Sweref> {
        // http://www.cs.swan.ac.uk/~cssimon/line_intersection.html
        let x1 = self.p0.east;
        let x2 = self.p1.east;
        let x3 = other.p0.east;
        let x4 = other.p1.east;
        let y1 = self.p0.north;
        let y2 = self.p1.north;
        let y3 = other.p0.north;
        let y4 = other.p1.north;
        
        let n = (x4 - x3)*(y1 - y2) - (x1 - x2)*(y4 - y3);

        let ta = ((y3 - y4)*(x1 - x3) + (x4 - x3)*(y1 - y3))/n;
        let tb = ((y1 - y2)*(x1 - x3) + (x2 - x1)*(y1 - y3))/n;

        // if n was zero, 
        if ta.is_infinite() { return None };

        match ta >= 0f64 && ta <= 1f64 && tb >= 0f64 && tb <= 1f64 {
            true => Some(Sweref { north: y1 + ta * (y2 - y1), east: x1 + ta * (x2 - x1) }),
            false => None,
        }
        // match (ta.partial_cmp(&0f64), ta.partial_cmp(&1f64), tb.partial_cmp(&0f64), tb.partial_cmp(&1f64)) {
        //     (Some(Ordering::Greater), Some(Ordering::Less), Some(Ordering::Greater), Some(Ordering::Less)) => 
        //         Some(Sweref { north: y1 + ta * (y2 - y1), east: x1 + ta * (x2 - x1) }),
        //     _ => None,
        // }
    }
}

