use nalgebra::Matrix3;

/*  β1x + β2y + β3 = z

    | x1 y1 1 |
    | x2 y2 1 |
X = | x3 y3 1 |  (M = number of points, N = 3)
    | x4 y4 1 |
    | x5 y5 1 |
    | z1 |
    | z1 |
Y = | z1 | (M = number of points, N = 1)
    | z1 |
    | z1 |
    | F1 |
β = | F2 | (M = 3, N = 1)
    | F3 |
Y = Xβ
OLS: β_opt = (XT X)^(-1) XT Y
3xC * Cx3 => 3x3
3x3 * 3xC => 3xC
3xC * Cx1 => 3x1
*/

// Adapted from: https://www.ilikebigbits.com/2017_09_25_plane_from_points_2.html

// Fit a plane to a collection of points.
// Fast, and accurate to within a few degrees.
// Returns None if the points do not span a plane.
fn plane_from_points(points: &[Point3D]) -> Option<(Point3D, [f64;3])>  {
    let n = points.len();
    if n < 3 {
        return None;
    }

    let mut sum = Vec3{x:0.0, y:0.0, z:0.0};
    for p in points {
        sum = &sum + &p;
    }
    let centroid = &sum * (1.0 / (n as f64));

    // Calculate full 3x3 covariance matrix, excluding symmetries:
    let mut xx = 0.0; let mut xy = 0.0; let mut xz = 0.0;
    let mut yy = 0.0; let mut yz = 0.0; let mut zz = 0.0;

    for p in points {
        let r = p - centroid;
        xx += r.x * r.x;
        xy += r.x * r.y;
        xz += r.x * r.z;
        yy += r.y * r.y;
        yz += r.y * r.z;
        zz += r.z * r.z;
    }

    xx /= n as f64;
    xy /= n as f64;
    xz /= n as f64;
    yy /= n as f64;
    yz /= n as f64;
    zz /= n as f64;

    let mut weighted_dir = Vec3{x: 0.0, y: 0.0, z: 0.0};

    {
        let det_x = yy*zz - yz*yz;
        let axis_dir = Vec3{
            x: det_x,
            y: xz*yz - xy*zz,
            z: xy*yz - xz*yy,
        };
        let mut weight = det_x * det_x;
        if weighted_dir.dot(&axis_dir) < 0.0 { weight = -weight; }
        weighted_dir += &axis_dir * weight;
    }

    {
        let det_y = xx*zz - xz*xz;
        let axis_dir = Vec3{
            x: xz*yz - xy*zz,
            y: det_y,
            z: xy*xz - yz*xx,
        };
        let mut weight = det_y * det_y;
        if weighted_dir.dot(&axis_dir) < 0.0 { weight = -weight; }
        weighted_dir += &axis_dir * weight;
    }

    {
        let det_z = xx*yy - xy*xy;
        let axis_dir = Vec3{
            x: xy*yz - xz*yy,
            y: xy*xz - yz*xx,
            z: det_z,
        };
        let mut weight = det_z * det_z;
        if weighted_dir.dot(&axis_dir) < 0.0 { weight = -weight; }
        weighted_dir += &axis_dir * weight;
    }

    let normal = normalize(&weighted_dir);
    if normal.is_finite() {
        Some(plane_from_point_and_normal(centroid, normal))
    } else {
        None
    }
}