use crate::geometry::{Point3D,Bounds};

#[derive(Clone,Hash,Eq,PartialEq,Debug)]
pub struct HexGridPosition {
    pub x: u32,
    pub y: u32,
}

// The hex grid has an origin point and a center-to-center distance.

#[derive(Clone,Debug)]
pub struct HexGrid {
    origin: Point3D,
    pub size: f64,
}

pub struct HexGridIterator {
    grid: HexGrid,
    bounds: Bounds,

    position: HexGridPosition,

    min_x: u32, max_x: u32, max_y: u32,
}

impl HexGrid {
    pub fn iter_over_points_in_bounds(&self, bounds: &Bounds) -> HexGridIterator {
        let min_x = ((bounds.lower.x - self.origin.x) / self.size).floor() as u32;
        let max_x = ((bounds.upper.x - self.origin.x) / self.size).ceil() as u32;
        let min_y = ((bounds.lower.y - self.origin.y) / self.size).floor() as u32;
        let max_y = ((bounds.upper.y - self.origin.y) / self.size).ceil() as u32;

        assert!(min_x > 0 && min_y > 0, "The origin for the grid must be chosen so that our indices are always positive, with some margin");

        HexGridIterator { grid: self.clone(), bounds: bounds.clone(), 
            position: HexGridPosition { x: min_x, y: min_y, },
            min_x, max_x, max_y,
        }
    }

    pub fn covering_bounds(bounds: &Bounds, size: f64) -> HexGrid {
        const EXTRA_MARGIN_MULTIPLIER: f64 = 3.0;
        let my_bounds = bounds.outset_by(size * EXTRA_MARGIN_MULTIPLIER);
        HexGrid {
            origin: my_bounds.lower,
            size,
        }
    }
}

impl Iterator for HexGridIterator {
    type Item = (HexGridPosition, Point3D);

    fn next(&mut self) -> Option<(HexGridPosition, Point3D)> {
        
        self.position = if self.position.x + 1 > self.max_x {
            if self.position.y + 1 > self.max_y {
                return None;
            } else {
                HexGridPosition { x: self.min_x, y: self.position.y + 1, }
            }
        } else {
            HexGridPosition { x: self.position.x + 1, y: self.position.y, }
        };
        let odd = (self.position.y % 2) == 1;
        let p = Point3D {
            x: self.grid.origin.x + self.grid.size * ((self.position.x as f64) + if odd { 0.5 } else { 0.0 }),
            y: self.grid.origin.y + self.grid.size * (self.position.y as f64),
            z: self.grid.origin.z,
        };
        if !self.bounds.contains_2d(&p) {
            // if self.position.x != self.min_x && self.position.x != self.max_x && self.position.y != self.max_y {
            //     println!("Position: {:?} -> {:?} not in {:?}", self.position, p, self.bounds);
            // }
            self.next()
        } else {
            Some((self.position.clone(), p))
        }
    }
}
