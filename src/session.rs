use std::{collections::HashMap, time::{Duration, Instant}};

#[derive(Clone)]
pub struct Session { pub data: HashMap<String, String>, pub exp: Instant }

pub struct SessionStore { ttl: Duration, map: HashMap<String, Session> }

impl SessionStore {
    pub fn new_ttl(sec: u64) -> Self { Self { ttl: Duration::from_secs(sec), map: HashMap::new() } }

    pub fn touch(&mut self, id: &str) { if let Some(s) = self.map.get_mut(id) { s.exp = Instant::now() + self.ttl; } }

    pub fn get_or_create(&mut self, id_opt: Option<&str>) -> (String, &mut Session) {
        let id = id_opt.map(str::to_string).unwrap_or(crate::util::gen_token(24));
        if !self.map.contains_key(&id) {
            self.map.insert(id.clone(), Session{ data: HashMap::new(), exp: Instant::now() + self.ttl });
        }
        let ttl = self.ttl;
        let s = self.map.get_mut(&id).unwrap();
        s.exp = std::time::Instant::now() + ttl;
        (id, s)
    }

    pub fn gc(&mut self) {
        let now = Instant::now();
        self.map.retain(|_, s| s.exp > now);
    }
}
