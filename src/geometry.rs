use super::sweref_to_wgs84::Sweref;

#[derive(Clone,Copy,Debug)]
pub struct Rectangle {
    pub southwest: Sweref, 
    pub northeast: Sweref,
}

impl Rectangle {

    pub fn from_points(pts: &Vec<Sweref>) -> Self {
        Self {
            southwest: Sweref {
                east: pts.iter().map(|p| p.east).fold(0./0., f64::min),
                north: pts.iter().map(|p| p.north).fold(0./0., f64::min),
            },
            northeast: Sweref {
                east: pts.iter().map(|p| p.east).fold(0./0., f64::max),
                north: pts.iter().map(|p| p.north).fold(0./0., f64::max),
            }
        }
    }

    pub fn create(xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Self {
        Self {
            southwest: Sweref { east: xmin, north: ymin, },
            northeast: Sweref { east: xmax, north: ymax, },
        }
    }

    pub fn northwest(&self) -> Sweref { Sweref { north: self.northeast.north, east: self.southwest.east } }
    pub fn southeast(&self) -> Sweref { Sweref { north: self.southwest.north, east: self.northeast.east } }

    pub fn middle(&self) -> Sweref { 
        Sweref {
            north: (self.southwest.north + self.northeast.north) * 0.5,
            east: (self.southwest.east + self.northeast.east) * 0.5,
        }
    }
    
    pub fn segments(&self) -> Vec<LineSegment> {

        let nw = self.northwest();
        let se = self.southeast();
        vec![
            LineSegment { p0: self.southwest.clone(), p1: se.clone() },
            LineSegment { p0: se.clone(), p1: self.northeast.clone() },
            LineSegment { p0: self.northeast.clone(), p1: nw.clone() },
            LineSegment { p0: nw.clone(), p1: self.southwest.clone() },
        ]
    }

    pub fn contains(&self, p: &Sweref) -> bool {
        p.east <= self.northeast.east && 
        p.east >= self.southwest.east && 
        p.north >= self.southwest.north &&
        p.north <= self.northeast.north
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.contains(&other.southwest) || 
        self.contains(&other.southeast()) || 
        self.contains(&other.northeast) ||
        self.contains(&other.northwest())
    }

}

pub struct LineSegment {
    pub p0: Sweref,
    pub p1: Sweref,
}

impl LineSegment {

    pub fn create(a: &Sweref, b: &Sweref) -> LineSegment {
        LineSegment { p0: a.clone(), p1: b.clone() }
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

        let intersects = 0.0f64..1.0f64;
        match intersects.contains(&ta) && intersects.contains(&tb) {
            true => Some(Sweref { north: y1 + ta * (y2 - y1), east: x1 + ta * (x2 - x1) }),
            false => None,
        }
    }
}

