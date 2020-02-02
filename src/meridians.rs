use std::sync::mpsc::Sender;
use super::ocad::{self, GraphSymbol};
use super::geometry;
use super::sweref_to_wgs84::Sweref;
use colored::*;

const MERIDIAN_SPACING: f64 = 300.0;

pub fn add_meridians(bounding_box: &geometry::Rectangle,
    rotation_angle: f64,
    file: &Sender<ocad::Object>, 
    verbose: bool) {

    if verbose {
        println!("[{}] Adding meridians at {} m spacing.", "MISC".magenta(), MERIDIAN_SPACING);
    }

    let middle = bounding_box.middle();
    let corners = vec![bounding_box.southwest, bounding_box.southeast(), bounding_box.northeast, bounding_box.northwest()];
    let c = f64::cos(rotation_angle.to_radians());
    let s = f64::sin(rotation_angle.to_radians());
    let rotate = |p: &Sweref| -> Sweref { Sweref {
        north: -s * (p.east - middle.east) + c * (p.north - middle.north) + middle.north,
        east: c * (p.east-middle.east) + s * (p.north - middle.north) + middle.east,
    }};

    let rotated_corners = corners.iter().map(rotate).collect();
    let rotated_bounding_box = geometry::Rectangle::from_points(&rotated_corners);

    let mut x = rotated_bounding_box.southwest.east;
    let mut meridians = Vec::new();
    while x < rotated_bounding_box.northeast.east {
        meridians.push(vec![
            rotate(&Sweref { east: x, north: rotated_bounding_box.southwest.north, }),
            rotate(&Sweref { east: x, north: rotated_bounding_box.northeast.north, }),
        ]);
        x = x + MERIDIAN_SPACING;
    }
    ocad::post_objects_without_clipping(meridians, &vec![GraphSymbol::Stroke(601000,false)], file);
}