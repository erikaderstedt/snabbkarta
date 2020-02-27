use super::dtm::DigitalTerrainModel;

pub fn rain_on(dtm: &mut DigitalTerrainModel) {

    let mut water_per_triangle: Vec<f64> = dtm.areas.clone();
    let mut integrated_water_per_triangle = vec![0f64;dtm.num_triangles];
    let normals = dtm.normals();

    loop {
        // For each triangle, calculate the flow to neighbouring triangles.
        let mut flow = vec![0f64;dtm.num_triangles];

        for (triangle,water) in water_per_triangle.iter().enumerate() {

        }

        for (triangle, delta_water) in flow.iter().enumerate() {
            water_per_triangle[triangle] = water_per_triangle[triangle] + delta_water;
            integrated_water_per_triangle[triangle] = integrated_water_per_triangle[triangle] + water_per_triangle[triangle];
        }
        
    }

}