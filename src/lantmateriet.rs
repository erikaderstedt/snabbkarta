use super::shapefiles::SurveyAuthorityConfiguration;
use super::ocad::GraphSymbol::{self,Stroke,Fill};

// https://www.lantmateriet.se/globalassets/kartor-och-geografisk-information/kartor/fastshmi.pdf

pub struct LantmaterietShapes { }

impl SurveyAuthorityConfiguration for LantmaterietShapes {

    fn supports_file(&self, s:&str) -> bool {
        match &s[0..2] {
            "vl" | "kl" | "bl" | "hl" | "oh" | "vo" | "ml" | "ma" | "mb" | "my" | "ms" => true,
            _ => false,
        }
    }

    fn symbol_from_detaljtyp(&self, s: &str, detaljtyp: &str) -> Option<GraphSymbol> {
        match &s[0..2] {
            // Linjeskikt med vägar
            "vl" => match &detaljtyp[3..5] {
                "A1" | "A2" | "A3" | "AS" | "BN" | "KV" | "MO" => Some(Stroke(502000,false)),
                "BS" => Some(Stroke(503000,false)),
                "JV" => Some(Stroke(509000,false)),
                _ => None, },
            
            // Linjeskikt med kraftledningar
            "kl" => Some(Stroke(510000,false)),

            // Linjeskikt med byggnader (används inte längre 2020)
            // Ytskikt med byggnader
            "bl" | "by" => Some(Fill(521000)),

            // Linjeskikt med hydrografi
            "hl" => Some(Stroke(304000,false)),

            // Höjdkurvor  
            "oh" => Some(Stroke(101000,false)),

            // Linjeskikt med övriga vägar
            "vo" => match &detaljtyp[4..7] {
                "TRA" => Some(Stroke(504000,false)),
                "CYK" => Some(Stroke(503000,false)),
                "STI" => Some(Stroke(506000,false)),
                "ELS" => Some(Stroke(505000,false)),
                _ => None, },

            // Linjeskikt med markdata
            "ml" => match detaljtyp {
                "ODLMARK.B" => Some(Stroke(415000,false)),
                "BEBOMR.B" => Some(Stroke(521001,false)),
                _ => None, },

            // Ytskikt för odlad mark
            "ma" => match detaljtyp {
                "ODLÅKER" => Some(Fill(412000)),
                "ODLFRUKT" => Some(Fill(413000)),
                _ => None, },    

            // Ytskikt med heltäckande markdata
            "my" => match detaljtyp {
                "ODLÅKER" => Some(Fill(412000)),
                "ODLFRUKT" => Some(Fill(413000)),
                "ÖPMARK" => Some(Fill(401000)),
                "ÖPTORG" => Some(Fill(501000)),
                _ => None, },

            // Ytskikt med bebyggelse
            "mb" => match detaljtyp {
                "ÖPTORG" => Some(Fill(501000)),
                _ => Some(Fill(520000)),
            },
    
            // Ytskikt med sankmark  
            "ms" => match detaljtyp {
                "SANK" => Some(Fill(308000)),
                _ => Some(Fill(307000)),
            },

            _ => None,
        }
    }

}