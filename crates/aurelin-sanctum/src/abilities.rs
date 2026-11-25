/// The abilities granted to a personal access token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Abilities(Vec<String>);

impl Default for Abilities {
    fn default() -> Self {
        Self(vec!["*".into()])
    }
}

impl Abilities {
    pub fn new(abilities: Vec<String>) -> Self {
        Self(abilities)
    }

    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    pub fn allows(&self, ability: &str) -> bool {
        self.0.iter().any(|granted| granted == "*" || granted == ability)
    }

    pub fn allows_all(&self, abilities: &[&str]) -> bool {
        abilities.iter().all(|ability| self.allows(ability))
    }

    pub fn allows_any(&self, abilities: &[&str]) -> bool {
        abilities.iter().any(|ability| self.allows(ability))
    }
}
