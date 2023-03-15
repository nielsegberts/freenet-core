#![allow(non_snake_case)]
use std::{borrow::Cow, cell::RefCell, rc::Rc};

use chrono::Utc;
use dioxus::prelude::*;
use freenet_email_inbox::InboxParams;
use locutus_aft_interface::TokenAssignment;
use locutus_stdlib::prelude::ContractKey;
use once_cell::unsync::Lazy;
use rsa::{pkcs1::DecodeRsaPrivateKey, RsaPrivateKey, RsaPublicKey};

use crate::inbox::{DecryptedMessage, InboxModel, MessageModel};

mod login;

#[cfg(all(feature = "node", target_arch = "wasm32"))]
thread_local! {
    static CONNECTION: Lazy<Rc<RefCell<crate::WebApi>>> = Lazy::new(|| {
            let api = crate::WebApi::new()
                .map_err(|err| {
                    web_sys::console::error_1(&serde_wasm_bindgen::to_value(&err).unwrap());
                    err
                })
                .expect("open connection");
            Rc::new(RefCell::new(api))
    })
}

#[cfg(all(feature = "node", not(target_arch = "wasm32")))]
thread_local! {
    static CONNECTION: Lazy<Rc<RefCell<crate::WebApi>>> = Lazy::new(|| {
        unimplemented!()
    })
}

#[derive(Debug, Clone)]
struct Inbox {
    inbox_ids: Vec<Identity>,
    inbox_data: Vec<Rc<RefCell<InboxModel>>>,
    messages: Rc<RefCell<Vec<Message>>>,
    active_id: usize,
}

static INBOX_CODE_HASH: &str = include_str!("./inbox_key");

impl Inbox {
    fn new(
        cx: Scope,
        contracts: Vec<Identity>,
        private_key: &rsa::RsaPrivateKey,
    ) -> Result<Self, String> {
        let mut models = Vec::with_capacity(contracts.len());
        #[cfg(feature = "node")]
        {
            for contract in &contracts {
                let private_key = private_key.clone();
                let model = CONNECTION.with(|conn| {
                    let params = InboxParams {
                        pub_key: contract.pub_key.clone(),
                    }
                    .try_into()
                    .map_err(|e| format!("{e}"))?;
                    let contract_key =
                        ContractKey::decode(INBOX_CODE_HASH, params).map_err(|e| format!("{e}"))?;
                    let client = (**conn).clone();
                    let f = use_future(cx, (), |_| async move {
                        let client = &mut *client.borrow_mut();
                        InboxModel::get_inbox(client, &private_key, contract_key).await
                    });
                    let inbox = loop {
                        match f.value() {
                            Some(v) => break v.as_ref().unwrap(),
                            None => std::thread::sleep(std::time::Duration::from_micros(100)),
                        }
                    };
                    Ok::<_, String>(inbox.clone())
                })?;
                models.push(Rc::new(RefCell::new(model)));
            }
        }
        Ok(Self {
            inbox_data: models,
            inbox_ids: contracts,
            messages: Rc::new(RefCell::new(vec![])),
            active_id: 0,
        })
    }

    fn send_message(
        &self,
        cx: Scope,
        to: &str,
        title: &str,
        content: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let inbox = self.inbox_data[self.active_id].clone();
        {
            let content = DecryptedMessage {
                title: title.to_owned(),
                content: title.to_owned(),
                from: "".to_owned(),
                to: vec![to.to_owned()],
                cc: vec![],
                time: Utc::now(),
            };
            // todo: request the token assignment to the component here
            let token = TokenAssignment {
                tier: todo!(),
                time_slot: todo!(),
                assignee: todo!(),
                signature: todo!(),
                assignment_hash: todo!(),
                token_record: todo!(),
            };
            inbox.borrow_mut().add_received_message(content, token);
        }
        #[cfg(feature = "node")]
        {
            CONNECTION.with(|conn| {
                let id = &self.inbox_ids[self.active_id];
                let client = (**conn).clone();
                let f = use_future(cx, (), |_| async move {
                    let client = &mut *client.borrow_mut();
                    inbox
                        .borrow_mut()
                        .add_messages_to_store(client, &[0])
                        .await?;
                    Ok::<_, Box<dyn std::error::Error>>(())
                });
                loop {
                    match f.value() {
                        Some(Err(e)) => {
                            break Err::<(), Box<dyn std::error::Error>>(format!("{e}").into())
                        }
                        Some(_) => {}
                        None => std::thread::sleep(std::time::Duration::from_micros(100)),
                    }
                }
            })?;
        }
        Ok(())
    }

    #[cfg(feature = "ui-testing")]
    fn load_messages(&self, _cx: Scope, id: &Identity, _private_key: &rsa::RsaPrivateKey) {
        let emails = {
            if id.id == 0 {
                vec![
                    Message {
                        id: 0,
                        from: "Ian's Other Account".into(),
                        title: "Unread email from Ian's Other Account".into(),
                        content: "Lorem ipsum dolor sit amet, consectetur adipiscing elit..."
                            .repeat(10)
                            .into(),
                        read: false,
                    },
                    Message {
                        id: 1,
                        from: "Mary".to_string().into(),
                        title: "Email from Mary".to_string().into(),
                        content: "Lorem ipsum dolor sit amet, consectetur adipiscing elit..."
                            .repeat(10)
                            .into(),
                        read: true,
                    },
                ]
            } else {
                vec![
                    Message {
                        id: 0,
                        from: "Ian Clarke".into(),
                        title: "Unread email from Ian".into(),
                        content: "Lorem ipsum dolor sit amet, consectetur adipiscing elit..."
                            .repeat(10)
                            .into(),
                        read: false,
                    },
                    Message {
                        id: 1,
                        from: "Jane".to_string().into(),
                        title: "Email from Jane".to_string().into(),
                        content: "Lorem ipsum dolor sit amet, consectetur adipiscing elit..."
                            .repeat(10)
                            .into(),
                        read: true,
                    },
                ]
            }
        };
        self.messages.replace(emails);
    }

    #[cfg(all(feature = "node", not(feature = "ui-testing")))]
    fn load_messages(&self, cx: Scope, id: &Identity, private_key: &rsa::RsaPrivateKey) {
        CONNECTION.with(|conn| {
            let private_key = private_key.clone();
            let key = self
                .contracts
                .iter()
                .find(|c| c.id == id.id)
                .unwrap()
                .pub_key
                .clone();
            let contract_key = todo!("get the id, from the code + params");
            let client = (**conn).clone();
            let f = use_future(cx, (), |_| async move {
                let client = &mut *client.borrow_mut();
                InboxModel::get_inbox(client, &private_key, contract_key).await
            });
            let inbox = loop {
                match f.value() {
                    Some(v) => break v.as_ref().unwrap(),
                    None => std::thread::sleep(std::time::Duration::from_millis(100)),
                }
            };
            let messages = &mut *self.messages.borrow_mut();
            messages.clear();
            messages.extend(inbox.messages.iter().map(|m| m.clone().into()));
        })
    }
}

struct User {
    logged: bool,
    active_id: Option<usize>,
    identities: Vec<Identity>,
    private_key: Option<RsaPrivateKey>,
}

impl User {
    #[cfg(feature = "ui-testing")]
    fn new() -> Self {
        const RSA_4096_PRIV_PEM: &str = include_str!("../examples/rsa4096-id-1-priv.pem");
        let priv_key = RsaPrivateKey::from_pkcs1_pem(RSA_4096_PRIV_PEM).unwrap();
        User {
            logged: true,
            active_id: None,
            identities: vec![
                Identity {
                    id: 0,
                    pub_key: priv_key.to_public_key(),
                },
                Identity {
                    id: 1,
                    pub_key: priv_key.to_public_key(),
                },
            ],
            private_key: Some(priv_key),
        }
    }

    fn logged_id(&self) -> Option<&Identity> {
        self.active_id.and_then(|id| self.identities.get(id))
    }

    fn set_logged_id(&mut self, id: usize) {
        assert!(id < self.identities.len());
        self.active_id = Some(id);
    }
}

#[derive(Clone, Debug)]
struct Identity {
    id: usize,
    pub_key: RsaPublicKey,
}

#[derive(Debug, Clone, Eq, Props)]
struct Message {
    id: u64,
    from: Cow<'static, str>,
    title: Cow<'static, str>,
    content: Cow<'static, str>,
    read: bool,
}

impl From<MessageModel> for Message {
    fn from(value: MessageModel) -> Self {
        Message {
            id: value.id,
            from: value.content.from.into(),
            title: value.content.title.into(),
            content: value.content.content.into(),
            read: false,
        }
    }
}

impl PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialOrd for Message {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl Ord for Message {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

pub(crate) fn App(cx: Scope) -> Element {
    // #[cfg(target_arch = "wasm32")]
    // {
    //     web_sys::console::log_1(&serde_wasm_bindgen::to_value("Starting app...").unwrap());
    // }
    // TODO: in the future this will be dinamically loaded from the identity component
    let contracts = vec![
        Identity {
            id: 0,
            pub_key: {
                const RSA_PRIV_PEM: &str = include_str!("../examples/rsa4096-id-0-priv.pem");
                RsaPrivateKey::from_pkcs1_pem(RSA_PRIV_PEM)
                    .unwrap()
                    .to_public_key()
            },
        },
        Identity {
            id: 1,
            pub_key: {
                const RSA_PRIV_PEM: &str = include_str!("../examples/rsa4096-id-1-priv.pem");
                RsaPrivateKey::from_pkcs1_pem(RSA_PRIV_PEM)
                    .unwrap()
                    .to_public_key()
            },
        },
    ];
    use_shared_state_provider(cx, User::new);
    use_context_provider(cx, || {
        const RSA_PRIV_PEM: &str = include_str!("../examples/rsa4096-user-priv.pem");
        let key = RsaPrivateKey::from_pkcs1_pem(RSA_PRIV_PEM).unwrap();
        Inbox::new(cx, contracts, &key).unwrap()
    });

    let user = use_shared_state::<User>(cx).unwrap();
    let user = user.read();
    if let Some(id) = user.logged_id() {
        let inbox = use_context::<Inbox>(cx).unwrap();
        inbox.load_messages(cx, id, user.private_key.as_ref().unwrap());
        cx.render(rsx! {
           UserInbox {}
        })
    } else {
        cx.render(rsx! {
           login::LoggingScreen {}
        })
    }
}

#[derive(Default)]
struct MenuSelection {
    email: Option<u64>,
    new_msg: bool,
}

impl MenuSelection {
    fn set_new_msg(&mut self) {
        if self.new_msg {
            self.new_msg = false;
        } else {
            self.new_msg = true;
            self.email = None;
        }
    }

    fn new_msg(&self) -> bool {
        self.new_msg
    }

    fn set_received(&mut self) {
        self.email = None;
        self.new_msg = false;
    }

    fn received(&self) -> bool {
        !self.new_msg && self.email.is_none()
    }
}

fn UserInbox(cx: Scope) -> Element {
    use_shared_state_provider(cx, MenuSelection::default);
    cx.render(rsx!(
        div {
            class: "columns",
            nav {
                class: "column is-one-fifth menu",
                UserMenuComponent {}
            }
            div {
                class: "column",
                InboxComponent {}
            }
        }
    ))
}

fn UserMenuComponent(cx: Scope) -> Element {
    let user = use_shared_state::<User>(cx).unwrap();
    let menu_selection = use_shared_state::<MenuSelection>(cx).unwrap();

    let received_class = (menu_selection.read().received() || !menu_selection.read().new_msg())
        .then(|| "is-active")
        .unwrap_or("");
    let write_msg_class = menu_selection
        .read()
        .new_msg()
        .then(|| "is-active")
        .unwrap_or("");

    cx.render(rsx!(
        div {
            class: "pl-3 pr-3 mt-3",
            ul {
                class: "menu-list",
                li {
                    a {
                        class: received_class,
                        onclick: move |_| { menu_selection.write().set_received(); },
                        "Received"
                    }
                }
                li {
                    a {
                        class: write_msg_class,
                        onclick: move |_| {
                            let mut selection = menu_selection.write();
                            selection.set_new_msg();
                        },
                        "Write message"
                    }
                }
                li {
                    a {
                        onclick: move |_| {
                            let mut logged_state = user.write();
                            logged_state.logged = false;
                            logged_state.active_id = None;
                        },
                        "Log out"
                    }
                }
            }
        }
    ))
}

fn InboxComponent(cx: Scope) -> Element {
    let inbox = use_context::<Inbox>(cx).unwrap();
    let menu_selection = use_shared_state::<MenuSelection>(cx).unwrap();

    #[inline_props]
    fn EmailLink<'a>(
        cx: Scope<'a>,
        sender: Cow<'a, str>,
        title: Cow<'a, str>,
        read: bool,
        id: u64,
    ) -> Element {
        let open_mail = use_shared_state::<MenuSelection>(cx).unwrap();
        let icon_style = read
            .then(|| "fa-regular fa-envelope")
            .unwrap_or("fa-solid fa-envelope");
        cx.render(rsx!(a {
            class: "panel-block",
            id: "email-inbox-accessor-{id}",
            onclick: move |_| { open_mail.write().email = Some(*id); },
            span {
                class: "panel-icon",
                i { class: icon_style }
            }
            span { class: "ml-2", "{sender}" }
            span { class: "ml-5", "{title}" }
        }))
    }

    let emails = inbox.messages.borrow();

    let menu_selection_ref = menu_selection.read();
    let new_msg = menu_selection_ref.new_msg;
    if let Some(email_id) = menu_selection_ref.email {
        let id_p = (*emails).binary_search_by_key(&email_id, |e| e.id).unwrap();
        let email = &emails[id_p];
        cx.render(rsx! {
            OpenMessage {
                id: email.id,
                from: email.from.clone(),
                title: email.title.clone(),
                content: email.content.clone(),
                read: email.read,
            }
        })
    } else if new_msg {
        cx.render(rsx! {
            NewMessageWindow {}
        })
    } else {
        let links = emails.iter().map(|email| {
            rsx!(EmailLink {
                sender: email.from.clone(),
                title: email.title.clone()
                read: email.read,
                id: email.id,
            })
        });
        cx.render(rsx! {
            div {
                class: "panel is-link mt-3",
                p { class: "panel-heading", "Inbox" }
                p {
                    class: "panel-tabs",
                    a {
                        class: "is-active icon-text",
                        span { class: "icon", i { class: "fas fa-inbox" } }
                        span { "Primary" }
                    }
                    a {
                        class: "icon-text",
                        span { class: "icon",i { class: "fas fa-user-group" } },
                        span { "Social" }
                    }
                    a {
                        class: "icon-text",
                        span { class: "icon", i { class: "fas fa-circle-exclamation" } },
                        span { "Updates" }
                    }
                }
                div {
                    class: "panel-block",
                    p {
                        class: "control has-icons-left",
                        input { class: "input is-link", r#type: "text", placeholder: "Search" }
                        span { class: "icon is-left", i { class: "fas fa-search", aria_hidden: true } }
                    }
                }
                links
            }
        })
    }
}

fn OpenMessage(cx: Scope<Message>) -> Element {
    let menu_selection = use_shared_state::<MenuSelection>(cx).unwrap();
    let email = cx.props;
    cx.render(rsx! {
        div {
            class: "columns title mt-3",
            div {
                class: "column",
                a {
                    class: "icon is-small",
                    onclick: move |_| { menu_selection.write().email = None; },
                    i { class: "fa-sharp fa-solid fa-arrow-left", aria_label: "Back to Inbox", style: "color:#4a4a4a" }, 
                }
            }
            div { class: "column is-four-fifths", h2 { "{email.title}" } }
            div {
                class: "column", 
                a { class: "icon is-small", 
                onclick: move |_| { menu_selection.write().email = None; },
                i { class: "fa-sharp fa-solid fa-trash", aria_label: "Delete", style: "color:#4a4a4a" } } 
            }
        }
        div {
            id: "email-content-{email.id}",
            p {
                "{email.content}"
            }
        }
    })
}

fn NewMessageWindow(cx: Scope) -> Element {
    let menu_selection = use_shared_state::<MenuSelection>(cx).unwrap();
    let inbox = use_context::<Inbox>(cx).unwrap();
    let to = use_state(cx, String::new);
    let title = use_state(cx, String::new);
    let content = use_state(cx, String::new);
    cx.render(rsx! {
        div {
            class: "column mt-3",
            div {
                class: "box has-background-light",
                h3 { class: "title is-3", "New message" }
                table {
                    class: "table is-narrow has-background-light",
                    tbody {
                        tr {
                            th { "From" }
                            td { style: "width: 100%", br {} }
                        }
                        tr {
                            th { "To"}
                            td { style: "width: 100%", contenteditable: true, "{to}" }
                        }
                        tr {
                            th { "Title"}
                            td { style: "width: 100%", contenteditable: true, "{title}"  }
                        }
                    }
                }
            }
            div {
                class: "box",
                div {
                    contenteditable: true,
                    oninput: move |ev| { content.set(ev.value.clone()); },
                    br {}
                }
            }
            div {
                button {
                    class: "button is-info is-outlined",
                    onclick: move |_| {
                        inbox.send_message(cx, to.get(), title.get(), content.get()).expect("connection error");
                        menu_selection.write().set_new_msg();
                    },
                    "Send"
                }
            }
        }
    })
}
