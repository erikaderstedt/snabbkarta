use super::shapefiles::SurveyAuthorityConfiguration;
use super::ocad::GraphSymbol::{self,Stroke,Fill};
use super::geometry;
use dbase;
use super::sweref_to_wgs84::Sweref;

// https://www.lantmateriet.se/globalassets/kartor-och-geografisk-information/kartor/fastshmi.pdf

pub struct LantmaterietShapes { 
//    buildings: Vec<geometry::Rectangle>, // bounding boxes
}


impl SurveyAuthorityConfiguration for LantmaterietShapes {

    fn supports_file(&self, base_filename: &str) -> bool {
        match &base_filename[0..2] {
            "vl" | "kl" | "bl" | "hl" | "oh" | "vo" | "ml" | "ma" | "mb" | "my" | "ms" => true,
            _ => false,
        }
    }

    fn symbols_for_record(&self, base_filename: &str, 
//        item_bounding_box: geometry::Rectangle, 
        dbase_record: &dbase::Record) -> Vec<GraphSymbol> {

        let detaljtyp = match dbase_record.get("DETALJTYP").expect("No DETALJTYP field in record.") { 
            dbase::FieldValue::Character(s) if !s.is_none() => s.as_ref().unwrap().replace("�", "?"),
            _ => panic!("Invalid field value for DETALJTYP field."),
        };

        // Intersection of AY/FASTIGHET and MY/?PMARK, where there is a MB / BEBL?G, should be 520000 and bounded by
        // 520001.

        // Ask for all MB/BEBL?G first.
        // Store all AY/FASTIGHET, but don't output.
        // When MY/?PMARK is received, check if polygon intersects any MB/BEBL?G.
        // If so, intersect with AY/FASTIGHET. 

        // 



        // 

        match &base_filename[0..2] {
            // Linjeskikt med vägar
            "vl" => match &detaljtyp[3..5] {
                "A1" | "A2" | "A3" | "AS" | "BN" | "KV" | "MO" => vec![Stroke(502000,false)],
                "BS" => vec![Stroke(503000,false)],
                "JV" => vec![Stroke(509000,false)],
                _ => vec![], },
            
            // Linjeskikt med kraftledningar
            "kl" => vec![Stroke(510000,false)],

            // Linjeskikt med byggnader (används inte längre 2020)
            // Ytskikt med byggnader
            "bl" | "by" => vec![Fill(521000)],

            // Linjeskikt med hydrografi
            "hl" => vec![Stroke(304000,false)],

            // Höjdkurvor  
            "oh" => vec![Stroke(101000,false)],

            // Linjeskikt med övriga vägar
            "vo" => match &detaljtyp[4..7] {
                "TRA" => vec![Stroke(504000,false)],
                "CYK" => vec![Stroke(503000,false)],
                "STI" => vec![Stroke(506000,false)],
                "ELS" | "LED" => vec![Stroke(505000,false)],
                _ => { println!("{:?} {:?}", &base_filename[0..2], detaljtyp); vec![] }, },

            // Linjeskikt med markdata
            "ml" => match &detaljtyp[..] {
                "ODLMARK.B" => vec![Stroke(415000,false)],
                "BEBOMR.B" => vec![Stroke(521001,false)],                
                _ => vec![], },

            // Ytskikt för odlad mark
            "ma" => match &detaljtyp[..] {
                "ODL?KER" => vec![Fill(412000)],
                "ODLFRUKT" => vec![Fill(413000)],
                _ => { println!("{:?} {:?}", &base_filename[0..2], detaljtyp); vec![] }, },

            // Ytskikt med heltäckande markdata
            "my" => match &detaljtyp[..] {
                "ODL?KER" => vec![Fill(412000)],
                "ODLFRUKT" => vec![Fill(413000)],
                "BEBL?G" => vec![Fill(520000)],

                "?PMARK" => vec![Fill(401000),Stroke(516000, false)],
                "?PTORG" => vec![Fill(501000)],
                _ => { println!("{:?} {:?}", &base_filename[0..2], detaljtyp); vec![] }, },

            // Ytskikt med bebyggelse
            "mb" => match &detaljtyp[..] {
                "?PTORG" => vec![Fill(501000)],
                "BEBL?G" => vec![Fill(520000)],
                _ => vec![Fill(520000)],
            },

            "mo" => match &detaljtyp[..] {
                "?PMARK" => vec![Fill(403000)],
                _ => vec![],
            },
    
            // Ytskikt med sankmark  
            "ms" => match &detaljtyp[..] {
                "SANK" => vec![Fill(308000)],
                _ => vec![Fill(307000)],
            },

            _ => vec![],
        }
    }

}