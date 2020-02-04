use std::f64;

#[derive(Clone,Debug,Copy)]
pub struct Sweref {
    pub north: f64,
    pub east: f64
}

impl Sweref {

#[allow(non_snake_case)]

    pub fn from_wgs84(position: &Wgs84) -> Sweref {

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

#[derive(Debug)]
pub struct Wgs84 {
    pub latitude: f64,
    pub longitude: f64,
}

impl Wgs84 {

    pub fn from_lla(lla: &[f64;3]) -> Wgs84 { Wgs84 { latitude: lla[0], longitude: lla[1], } }

#[allow(non_snake_case)]

    pub fn from_sweref(position: &Sweref) -> Wgs84 {

        let k: f64 = 0.9996;  // Scale factor
        let cmeridian: f64 = 15.0f64.to_radians(); // UTM 33.
        
        let x = (position.east - 500000.0) / k;
        let y = position.north / k;

        let sm_a = 6378137.0f64;
        let sm_b = 6356752.314f64;
        
        /* Get the value of phif, the footpoint latitude. */
        let phif = footpoint_latitude (y);
        
        /* Precalculate ep2 */
        let ep2 = (sm_a.powi(2) - sm_b.powi(2)) / sm_b.powi(2);
        
        /* Precalculate cos (phif) */
        let cf = f64::cos (phif);
        
        /* Precalculate nuf2 */
        let nuf2 = ep2 * cf.powi(2);
        
        /* Precalculate Nf and initialize Nfpow */
        let Nf = sm_a.powi(2) / (sm_b * f64::sqrt (1.0 + nuf2));
        let mut Nfpow = Nf;
        
        /* Precalculate tf */
        let tf = f64::tan (phif);
        let tf2 = tf * tf;
        let tf4 = tf2 * tf2;
        
        /* Precalculate fractional coefficients for x**n in the equations
        below to simplify the expressions for latitude and longitude. */
        let x1frac = 1.0 / (Nfpow * cf);
        
        Nfpow *= Nf;   /* now equals Nf**2) */
        let x2frac = tf / (2.0 * Nfpow);
        
        Nfpow *= Nf;   /* now equals Nf**3) */
        let x3frac = 1.0 / (6.0 * Nfpow * cf);
        
        Nfpow *= Nf;   /* now equals Nf**4) */
        let x4frac = tf / (24.0 * Nfpow);
        
        Nfpow *= Nf;   /* now equals Nf**5) */
        let x5frac = 1.0 / (120.0 * Nfpow * cf);
        
        Nfpow *= Nf;   /* now equals Nf**6) */
        let x6frac = tf / (720.0 * Nfpow);
        
        Nfpow *= Nf;   /* now equals Nf**7) */
        let x7frac = 1.0 / (5040.0 * Nfpow * cf);
        
        Nfpow *= Nf;   /* now equals Nf**8) */
        let x8frac = tf / (40320.0 * Nfpow);
        
        /* Precalculate polynomial coefficients for x**n.
        -- x**1 does not have a polynomial coefficient. */
        let x2poly = -1.0 - nuf2;
        
        let x3poly = -1.0 - 2.0 * tf2 - nuf2;
        
        let x4poly = 5.0 + 3.0 * tf2 + 6.0 * nuf2 - 6.0 * tf2 * nuf2
        - 3.0 * (nuf2 *nuf2) - 9.0 * tf2 * (nuf2 * nuf2);
        
        let x5poly = 5.0 + 28.0 * tf2 + 24.0 * tf4 + 6.0 * nuf2 + 8.0 * tf2 * nuf2;
        
        let x6poly = -61.0 - 90.0 * tf2 - 45.0 * tf4 - 107.0 * nuf2
        + 162.0 * tf2 * nuf2;
        
        let x7poly = -61.0 - 662.0 * tf2 - 1320.0 * tf4 - 720.0 * (tf4 * tf2);
        
        let x8poly = 1385.0 + 3633.0 * tf2 + 4095.0 * tf4 + 1575.0 * (tf4 * tf2);
        
        /* Calculate latitude */
        let lat_rad = phif + x2frac * x2poly * (x * x)
        + x4frac * x4poly * x.powf(4.0)
        + x6frac * x6poly * x.powf(6.0)
        + x8frac * x8poly * x.powf(8.0);
        
        /* Calculate longitude */
        let lon_rad = cmeridian + x1frac * x
        + x3frac * x3poly * x.powf(3.0)
        + x5frac * x5poly * x.powf(5.0)
        + x7frac * x7poly * x.powf(7.0);
        
        Wgs84 {
            latitude: lat_rad.to_degrees(),
            longitude: lon_rad.to_degrees(),
        }
    }
}

fn footpoint_latitude(y: f64) -> f64 {
    let sm_a: f64 = 6378137.0;
    let sm_b: f64 = 6356752.314;
    
    /* Precalculate n (Eq. 10.18) */
    let n = (sm_a - sm_b) / (sm_a + sm_b);
    
    /* Precalculate alpha_ (Eq. 10.22) */
    /* (Same as alpha in Eq. 10.17) */
    let alpha_ = ((sm_a + sm_b) / 2.0)
    * (1.0 + n.powi(2) / 4.0 + n.powi(4) / 64.0);
    
    /* Precalculate y_ (Eq. 10.23) */
    let y_ = y / alpha_;
    
    /* Precalculate beta_ (Eq. 10.22) */
    let beta_ = 3.0 * n / 2.0 - 27.0 * n.powi(3) / 32.0 + 269.0 * n.powi(5) / 512.0;
    
    /* Precalculate gamma_ (Eq. 10.22) */
    let gamma_ = (21.0 * n.powi(2) / 16.0)
    + (-55.0 * n.powi(4) / 32.0);
    
    /* Precalculate delta_ (Eq. 10.22) */
    let delta_ = (151.0 * n.powi(3) / 96.0)
    + (-417.0 * n.powi(5) / 128.0);
    
    /* Precalculate epsilon_ (Eq. 10.22) */
    let epsilon_ = 1097.0 * n.powi(4) / 512.0;
    
    /* Now calculate the sum of the series (Eq. 10.21) */
    y_ + (beta_ * f64::sin(2.0 * y_))
    + (gamma_ * f64::sin(4.0 * y_))
    + (delta_ * f64::sin(6.0 * y_))
    + (epsilon_ * f64::sin(8.0 * y_))
}
