use std::f64;
use super::Wgs84;
use super::geometry::Point3D;
use geo::Point;

#[derive(Clone,Debug,Copy)]
pub struct Sweref {
    pub north: f64,
    pub east: f64
}

impl From<&Wgs84> for Sweref {

#[allow(non_snake_case)]
    fn from(position: &Wgs84) -> Self {

        let lat = position.latitude.to_radians();
        let lon = position.longitude.to_radians();

        let a: f64 = 6378137.0;
        let f: f64 = 1.0/298.257222101;
        let e2: f64 = f*(2.0-f);
        let n: f64 = f/(2.0-f);
        let at = a/(n+1.0) * (n.powi(2)/4.0+n.powi(4)/64.0 + 1.0);
        
        let A = e2;
        let B = 1.0/6.0 * (e2.powi(2)*5.0 - e2.powi(3));
        let C = 1.0/120.0 * (104.0*e2.powi(3) - 45.0*e2.powi(4));
        let D = 1.0/1260.0 * (1237.0*e2.powi(4));
        
        let long_av = 15.0f64.to_radians(); // Mid meridian for SWEREF99.
        let k = 0.9996;  // Scale factor
        let f_n = 0.0;      // False northing
        let f_e = 500000.0; // False easting
        
        let b1 = 0.5*n - 2.0/3.0*n.powi(2) + 5.0/16.0*n.powi(3) + 41.0/180.0*n.powi(4);
        let b2 = 13.0/48.0*n.powi(2) - 3.0/5.0*n.powi(3) + 557.0/1440.0*n.powi(4);
        let b3 = 61.0/240.0*n.powi(3) - 103.0/140.0*n.powi(4);
        let b4 = 49561.0/161280.0*n.powi(4);
    
        let d = lon - long_av;
        
        let s = f64::sin(lat);
        let lat1 = lat - s*f64::cos(lat)*(A + B*s.powi(2) + C*s.powi(4) + D*s.powi(6));
        let es = f64::atan(f64::tan(lat1)/f64::cos(d));
        let ns = f64::atanh(f64::cos(lat1)*f64::sin(d));

        Sweref { 
            north: k * at * (es + 
                b1*f64::sin(es*2.0)*f64::cosh(ns*2.0) + 
                b2*f64::sin(es*4.0)*f64::cosh(ns*4.0) +
                b3*f64::sin(es*6.0)*f64::cosh(ns*6.0) + 
                b4*f64::sin(es*8.0)*f64::cosh(ns*8.0)) + f_n,
            east:  k * at * (ns + 
                b1*f64::cos(es*2.0)*f64::sinh(ns*2.0) + 
                b2*f64::cos(es*4.0)*f64::sinh(ns*4.0) +
                b3*f64::cos(es*6.0)*f64::sinh(ns*6.0) + 
                b4*f64::cos(es*8.0)*f64::sinh(ns*8.0)) + f_e
        }
    }
}

impl From<&Point<f64>> for Sweref {
    fn from(c: &Point<f64>) -> Self {
        Sweref { east: c.x(), north: c.y() }
    }
}

impl From<&Point3D> for Sweref {
    fn from(c: &Point3D) -> Self {
        Sweref { east: c.x, north: c.y }
    }
}