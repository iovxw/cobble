use std::fmt::{self, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(untagged)]
pub enum Component {
    String(StringComponent),
    Translation(TranslationComponent),
    Keybind(KeybindComponent),
    Score(ScoreComponent),
    Selector(SelectorComponent),
}

impl fmt::Display for Component {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Component::String(v) => v.fmt(f),
            Component::Translation(v) => v.fmt(f),
            Component::Keybind(v) => fmt::Debug::fmt(v, f),
            Component::Score(v) => fmt::Debug::fmt(v, f),
            Component::Selector(v) => fmt::Debug::fmt(v, f),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(untagged)]
pub enum StringComponent {
    Raw(String),
    Mixed {
        text: String,
        #[serde(flatten)]
        fields: ComponentFields,
    },
}

impl fmt::Display for StringComponent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StringComponent::Raw(text) => f.write_str(text),
            StringComponent::Mixed { text, fields } => {
                f.write_str(&text)?;
                if let Some(extra) = &fields.extra {
                    for extra in extra {
                        extra.fmt(f)?;
                    }
                }
                Ok(())
            }
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct TranslationComponent {
    pub translate: String,
    pub with: Vec<Component>,
    #[serde(flatten)]
    pub fields: ComponentFields,
}

impl fmt::Display for TranslationComponent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]", self.translate)?;
        for component in &self.with {
            f.write_char(' ')?;
            component.fmt(f)?;
        }
        if let Some(extra) = &self.fields.extra {
            for extra in extra {
                extra.fmt(f)?;
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct KeybindComponent {
    pub keybind: String,
    #[serde(flatten)]
    pub fields: ComponentFields,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct ScoreComponent {
    pub score: Value,
    #[serde(flatten)]
    pub fields: ComponentFields,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct SelectorComponent {
    pub selector: Value,
    #[serde(flatten)]
    pub fields: ComponentFields,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComponentFields {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underlined: Option<bool>,
    pub strikethrough: Option<bool>,
    pub obfuscated: Option<bool>,
    pub color: Option<String>,
    pub insertion: Option<String>,
    pub click_event: Option<ClickEvent>,
    pub hover_event: Option<HoverEvent>,
    pub extra: Option<Vec<Component>>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case", tag = "action", content = "value")]
pub enum ClickEvent {
    OpenUrl(String),
    OpenFile(String),
    RunCommand(String),
    TwitchUserInfo(String),
    SuggestCommand(String),
    ChangePage(usize),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case", tag = "action", content = "value")]
pub enum HoverEvent {
    ShowText(Box<StringComponent>),
    ShowItem(Box<StringComponent>),
    ShowEntity(Box<StringComponent>),
    ShowAchievement(Box<StringComponent>),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn string_raw() {
        let input = r#""string""#;
        let expect = Component::String(StringComponent::Raw(r#"string"#.into()));

        let r: Component = serde_json::from_str(input).unwrap();
        assert_eq!(r, expect);
    }

    #[test]
    fn string_mixed() {
        let input = r#"{ "text": "string" }"#;
        let expect = StringComponent::Mixed {
            text: "string".into(),
            fields: ComponentFields::default(),
        };

        let r: StringComponent = serde_json::from_str(input).unwrap();
        assert_eq!(r, expect);
    }

    #[test]
    fn player_join() {
        let input = r#"
{
   "color":"yellow",
   "translate":"multiplayer.player.joined",
   "with":[
      {
         "insertion":"Username",
         "clickEvent":{
            "action":"suggest_command",
            "value":"/tell Username "
         },
         "hoverEvent":{
            "action":"show_entity",
            "value":{
               "text":"Hover"
            }
         },
         "text":"Username"
      }
   ]
}"#;
        let expect = Component::Translation(TranslationComponent {
            translate: "multiplayer.player.joined".into(),
            with: vec![Component::String(StringComponent::Mixed {
                text: "Username".into(),
                fields: ComponentFields {
                    insertion: Some("Username".into()),
                    click_event: Some(ClickEvent::SuggestCommand("/tell Username ".into())),
                    hover_event: Some(HoverEvent::ShowEntity(Box::new(StringComponent::Mixed {
                        text: "Hover".into(),
                        fields: ComponentFields::default(),
                    }))),
                    ..ComponentFields::default()
                },
            })],
            fields: ComponentFields {
                color: Some("yellow".into()),
                ..ComponentFields::default()
            },
        });
        let r: Component = serde_json::from_str(input).unwrap();
        assert_eq!(r, expect);
    }
}
