// The ml input data is:
// height (scaled to 1 at (min+max)/2, and going from 0 to 2)
// slope (difference between highest / lowest edge point in the hex, 0 - flat, 1 - up to 2.5 m difference)
// max height of vegetation returns in 5x5 area (divided by 20)
// contains LAS water points (0 or 1)
// number of ground returns vs num vegetation returns (averaged over a 5x5 m area), expressed as a quotient (vegetation returns / (ground returns + 1)).
// contains LAS building point (0 or 1)
//


use crate::dtm::DigitalTerrainModel;
use crate::las::PointDataRecord;
use crate::geometry::{Point3D,PointConverter,Bounds};
use crate::hexgrid::{HexGrid,HexGridPosition};
use std::collections::HashMap;
use std::f64;
use rayon::prelude::*;

struct CondensedRecord {
    point: Point3D,
    classification: u8,
}

#[repr(C)]
pub struct MachineLearningInputData {
    height: f32,
    slope: f32, // difference in max / min (in meters)
    max_height_of_other_points: f32,
    water_points: u16,  
    ground_points: u16,
    other_points: u16 
}

impl MachineLearningInputData {
    pub fn construct_hashmap( records: &Vec<PointDataRecord>, 
        point_converter: &PointConverter,
        dtm: &DigitalTerrainModel,
        hex_grid: &HexGrid) -> HashMap<HexGridPosition,MachineLearningInputData> {

        const NUM_X_SUB_DIVISIONS: usize = 4;
        const NUM_Y_SUB_DIVISIONS: usize = 4;
        const NUM_SUB_DIVISIONS: usize = NUM_X_SUB_DIVISIONS * NUM_Y_SUB_DIVISIONS;

        let full_bounds = &dtm.bounds;
        let dx = (full_bounds.upper.x - full_bounds.lower.x) / (NUM_X_SUB_DIVISIONS as f64);
        let dy = (full_bounds.upper.y - full_bounds.lower.y) / (NUM_Y_SUB_DIVISIONS as f64);

        println!("Full bounds {:?}", full_bounds);
        let mut output: HashMap<HexGridPosition,MachineLearningInputData> = HashMap::new();

        output.par_extend((0..NUM_SUB_DIVISIONS)
            .into_par_iter()
            .map(|i| -> HashMap<HexGridPosition,MachineLearningInputData> {
                let x_index = (i % NUM_X_SUB_DIVISIONS) as f64;
                let y_index = (i / NUM_X_SUB_DIVISIONS) as f64;
                let subset = Bounds {
                    lower: Point3D { 
                        x: full_bounds.lower.x + x_index * dx, 
                        y: full_bounds.lower.y + y_index * dy, 
                        z: full_bounds.lower.z, 
                    },
                    upper: Point3D {
                        x: full_bounds.lower.x + (x_index + 1f64) * dx,
                        y: full_bounds.lower.y + (y_index + 1f64) * dy,
                        z: full_bounds.upper.z,
                    }
                }.
                outset_by(hex_grid.size);
                construct_partial_ml_input_data(records, &point_converter, &subset, dtm, hex_grid)
            })
            .flatten()
        );
        output
    }
}

fn construct_partial_ml_input_data( records: &Vec<PointDataRecord>,
                            point_converter: &PointConverter,
                            subset: &Bounds, 
                            dtm: &DigitalTerrainModel,
                            hex_grid: &HexGrid) -> HashMap<HexGridPosition,MachineLearningInputData> {

    const NEARBY: f64 = 1.0f64;

    let low = point_converter.point_3d_to_record_coordinates(&subset.lower);
    let high = point_converter.point_3d_to_record_coordinates(&subset.upper);
    let records_within_bounds: Vec<&PointDataRecord> = records.iter().filter(|record| record.x >= low[0] && record.x <= high[0] && record.y >= low[1] && record.y <= high[1]).collect();

    let condensed_records: Vec<CondensedRecord> = records_within_bounds.iter()
        .map(|record| CondensedRecord { 
            point: point_converter.record_coordinates_to_point_3d(&[record.x, record.y, record.z]), 
            classification: record.classification 
        })
        .collect();

    let output: HashMap<HexGridPosition,MachineLearningInputData>
     = hex_grid.iter_over_points_in_bounds(subset).enumerate().map(|(i,(index, center))| -> (HexGridPosition, MachineLearningInputData) {
        if i%1000 == 0 {
            println!("{} for {:?}", i, subset);
        }
        let records_around_point: Vec<&CondensedRecord> = condensed_records
            .iter()
            .filter(|record| 
                record.point.x > center.x - NEARBY && 
                record.point.x < center.x + NEARBY && 
                record.point.y > center.y - NEARBY && 
                record.point.y < center.y + NEARBY)
            .collect();

        let ground_heights: Vec<f64> = records_around_point.iter().filter(|r| r.classification == 2).map(|r| r.point.z).collect();
        let ground_points = ground_heights.len();

        let slope: f64;
        let height: f64;

        if ground_points == 0 {
            // If there are no returns (like in parts of lakes) then we need to grab the height from the DTM.
            // We manually assign a slope of zero to these hexes.
            println!("Lake");
            // TODO: This is what takes time!
            height = dtm.z_coordinate_at_xy(&center); // TODO: include DTM
            slope = 0f64;
        } else {
            let lowest = ground_heights.iter().cloned().fold(0./0., f64::min);
            let highest = ground_heights.into_iter().fold(0./0., f64::max);
            height = (lowest + highest) * 0.5f64;
            slope = highest - lowest;
        }

        let water_points = records_around_point.iter().filter(|r| r.classification == 9).count() as u16;
        let other_points = records_around_point.iter().filter(|r| r.classification == 1).count() as u16; 

        (index, MachineLearningInputData {
            height: height as f32, 
            slope: slope as f32, 
            ground_points: ground_points as u16, 
            water_points, 
            other_points,
            max_height_of_other_points: 0f32,
        })
    }).collect();

    println!("{} points for bounds {:?}", output.len(), subset);
    output
}