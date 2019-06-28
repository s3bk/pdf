/// PDF "cryptography"

const PADDING: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41,
    0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08
    0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80,
    0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A
];

struct Decoder {
    key_len: usize
    key: [u8; 16] // maximum length
}
impl Decoder {
    fn default(dict: &Dictionary) -> Decoder {
        Decoder::from_password(dict, &b"")
    }
    fn from_password(dict: &Dictionary, pass: &[u8]) -> Result<Decoder> {
        // get important data first
        let o = dict.get("O")   
            .ok_or(PdfError::MissingEntry { typ: "Encrypt Dictionary", field: "O".into()})?
            as_string()?.as_bytes();
    
        let r = dict.get("R").as_integer()
            .ok_or(PdfError::MissingEntry { typ: "Encrypt Dictionary", field: "O".into()})? as u32;
        
        let level = 3;
        let key_size = 5;
        
        // a) and b)
        let mut hash = md5::Context::new();
        if pass.len() < 32 {
            hash.consume(pass);
            hash.consume(&PADDING[.. 32 - pass.len()]);
        } else {
            hash.consume(&pass[.. 32]);
        }
        
        // c)
        
        hash.consume(o);
        
        // d)
        
        hash.consume(r.to_le_bytes());
        
        // e) 
        
        // f) 
        if level >= 4 {
            hash.consume([0xff, 0xff, 0xff, 0xff]);
        }
        
        // g) 
        let mut data = *hash.compute();
        
        // h) 
        if level >= 3 {
            for _ in 0 .. 50 {
                data = md5::compute(&data[.. key_size]);
            }
        }
        
        
        
pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        

        if let Some(ref e) = self.trailer.encrypt_dict {
            match e.get("Filter").map(|s| s.as_str()) {
                Some("Standard") => match e.get("V").unwrap {
                    0 => bail!("undocumented encryption algorithm"),
                    1 | 2 => 
                    
                
    }
