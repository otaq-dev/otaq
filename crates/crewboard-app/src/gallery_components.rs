//! The gallery's M2 component sheets (T-0030): *Primitives* (Status Pill, Meta Chip), *Badges & Inputs* (badges,
//! tabs, segmented, toggle, fields, dropdown, checklist) and *Overlays & feedback* (tooltip, context menu,
//! confirm dialog), with live state so hover, focus, keyboard and clicks can be tried by hand.

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, IntoElement, SharedString, Window,
    div, prelude::*, px,
};

use crate::components::button::{Button, Variant};
use crate::components::dialog::ConfirmDialog;
use crate::components::field::{DropdownButton, Field};
use crate::components::menu::{ContextMenu, MenuChosen, Tooltip};
use crate::components::pill::{ChipTone, CountBadge, CountTone, MetaChip, StatusPill};
use crate::components::tabs::{Segmented, TabIndicator, TabItem, Tabs, Toggle};
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::IconName;
use crate::theme::{ActiveTheme, FONT_MONO, Theme, radius, space};
use crate::view_models::dialog::DialogKind;
use crate::view_models::fields::{FieldState, Rule, required_title};
use crate::view_models::menu::MenuItem;
use crewboard_core::domain::Status;

pub struct ComponentsDemo {
    title_empty: Entity<TextInput>,
    title: Entity<TextInput>,
    title_state: FieldState,
    env: Entity<TextInput>,
    body: Entity<TextInput>,
    disabled: Entity<TextInput>,
    tab: usize,
    segment: usize,
    toggles: [bool; 2],
    menu: Entity<ContextMenu>,
    chosen: Option<SharedString>,
    dialog: Option<DialogKind>,
    dialog_focus: FocusHandle,
    /// Collapsed epic cards, by variant index.
    collapsed: Vec<usize>,
}

const TITLE_RULES: &[Rule] = &[required_title];

pub fn menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("open", "Open panel")
            .icon(IconName::PanelRight)
            .shortcut("↵"),
        MenuItem::new("terminal", "Open terminal")
            .icon(IconName::SquareTerminal)
            .shortcut("⌘T"),
        MenuItem::new("diff", "Open diff")
            .icon(IconName::FileDiff)
            .shortcut("⌘D"),
        MenuItem::new("pin", "Pin to top")
            .icon(IconName::Pin)
            .checked(false)
            .separated(),
        MenuItem::new("copy", "Copy branch name")
            .icon(IconName::Copy)
            .shortcut("⇧⌘C"),
        MenuItem::new("chain", "Highlight chain")
            .icon(IconName::Link2)
            .shortcut("H"),
        MenuItem::new("send-back", "Send back…")
            .icon(IconName::Undo2)
            .shortcut("⌘⌫")
            .danger()
            .separated(),
        MenuItem::new("close", "Close ticket…")
            .icon(IconName::CircleSlash)
            .danger(),
        MenuItem::new("delete", "Delete…")
            .icon(IconName::Trash2)
            .danger()
            .disabled(false)
            .hint("Only for tickets that never started (0060)"),
    ]
}

impl ComponentsDemo {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title_empty =
            cx.new(|cx| TextInput::new(window, cx).placeholder("Short, imperative title"));
        let title = cx.new(|cx| {
            let mut input = TextInput::new(window, cx).placeholder("Short, imperative title");
            input.set_text("Checkout API", cx);
            input
        });
        cx.subscribe(&title, |demo, input, event: &InputEvent, cx| {
            let text = input.read(cx).text().to_owned();
            demo.title_state.set(text);
            if *event == InputEvent::Blur || *event == InputEvent::Submit {
                demo.title_state.touch();
            }
            let error = demo.title_state.shown_error(TITLE_RULES).is_some();
            input.update(cx, |i, cx| i.set_error(error, cx));
            cx.notify();
        })
        .detach();
        let env = cx.new(|cx| {
            let mut input = TextInput::new(window, cx).mono();
            input.set_text(".env.local", cx);
            input.set_error(true, cx);
            input
        });
        let body = cx.new(|cx| {
            let mut input = TextInput::new(window, cx).multiline(px(72.));
            input.set_text(
                "POST /api/checkout creates a Stripe session. Uses the client from SHOP-17.",
                cx,
            );
            input
        });
        let disabled = cx.new(|cx| {
            let mut input = TextInput::new(window, cx);
            input.set_text("Locked after start", cx);
            input.set_disabled(true, cx);
            input
        });
        let menu = cx.new(|cx| ContextMenu::new(menu_items(), window, cx).width(260.));
        cx.subscribe(&menu, |demo, _, chosen: &MenuChosen, cx| {
            demo.chosen = Some(chosen.0.clone());
            cx.notify();
        })
        .detach();
        cx.observe(&menu, |_, _, cx| cx.notify()).detach();
        Self {
            title_empty,
            title,
            title_state: FieldState::new("Checkout API"),
            env,
            body,
            disabled,
            tab: 0,
            segment: 0,
            toggles: [true, false],
            menu,
            chosen: None,
            dialog: None,
            dialog_focus: cx.focus_handle(),
            collapsed: Vec::new(),
        }
    }

    fn badges(&self, t: &Theme) -> AnyElement {
        let c = t.colors;
        let caption = |s: &str| {
            div()
                .font_family(FONT_MONO)
                .text_size(px(10.))
                .text_color(c.text_muted)
                .child(SharedString::from(s.to_owned()))
        };
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(caption("STATUS PILL · every status of SDD §7.2"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .children(Status::ALL.iter().map(|s| StatusPill::of(*s))),
            )
            .child(caption("COUNT"))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(CountBadge::new(7, CountTone::Attention))
                    .child(CountBadge::new(3, CountTone::Neutral))
                    .child(CountBadge::new(12, CountTone::Accent)),
            )
            .child(caption("DEPENDENCY · META"))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .child(MetaChip::new(IconName::Link2, "SHOP-18 SHOP-19"))
                    .child(MetaChip::new(
                        IconName::Hourglass,
                        "waits for SHOP-20 to merge",
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .child(MetaChip::new(
                        IconName::GitBranch,
                        crewboard_core::naming::branch("SHOP-18"),
                    ))
                    .child(MetaChip::new(IconName::Coins, "182k"))
                    .child(MetaChip::new(IconName::Play, ":4020").tone(ChipTone::Running)),
            )
            .child(caption("LAST SIGNAL"))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .child(MetaChip::new(IconName::Activity, "40 s ago"))
                    .child(MetaChip::new(IconName::Activity, "4 min ago").tone(ChipTone::Warning))
                    .child(MetaChip::new(IconName::GitPullRequest, "PR #481")),
            )
            .child(caption("LEGEND LABELS WITH LIVE PARTS"))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .child(StatusPill::legend(
                        crate::theme::Status::Running,
                        Some("Running :4020".into()),
                    ))
                    .child(StatusPill::legend(
                        crate::theme::Status::Conflicts,
                        Some("Conflicts with main".into()),
                    )),
            )
            .into_any_element()
    }

    fn tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let seg = cx.entity().downgrade();
        let tog = cx.entity().downgrade();
        let tog2 = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .w(px(600.))
            .child(
                Tabs::new(
                    "demo-tabs",
                    vec![
                        TabItem::new(IconName::SquareTerminal, "Terminal")
                            .indicator(TabIndicator::Live),
                        TabItem::new(IconName::FileDiff, "Diff")
                            .indicator(TabIndicator::Note("+412 −38".into())),
                        TabItem::new(IconName::Play, "Run")
                            .indicator(TabIndicator::Note(":4020".into())),
                        TabItem::new(IconName::Terminal, "Shell"),
                        TabItem::new(IconName::FileText, "Ticket"),
                        TabItem::new(IconName::History, "History")
                            .indicator(TabIndicator::Note("3".into())),
                    ],
                    self.tab,
                )
                .on_select(move |i, _, cx| {
                    let _ = this.update(cx, |d, cx| {
                        d.tab = i;
                        cx.notify();
                    });
                }),
            )
            .child(
                Segmented::new(
                    "demo-seg",
                    vec!["Unified".into(), "Side-by-side".into()],
                    self.segment,
                )
                .on_select(move |i, _, cx| {
                    let _ = seg.update(cx, |d, cx| {
                        d.segment = i;
                        cx.notify();
                    });
                }),
            )
            .child(
                div()
                    .flex()
                    .gap(px(20.))
                    .child(
                        Toggle::new("demo-toggle-on", self.toggles[0])
                            .label(if self.toggles[0] { "On" } else { "Off" })
                            .on_change(move |on, _, cx| {
                                let _ = tog.update(cx, |d, cx| {
                                    d.toggles[0] = on;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        Toggle::new("demo-toggle-off", self.toggles[1])
                            .label("Off · default")
                            .on_change(move |on, _, cx| {
                                let _ = tog2.update(cx, |d, cx| {
                                    d.toggles[1] = on;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        Toggle::new("demo-toggle-disabled", true)
                            .label("Disabled")
                            .disabled(true),
                    ),
            )
            .into_any_element()
    }

    fn fields(&self) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .w(px(292.))
            .child(Field::input("Title", &self.title_empty))
            .child(
                Field::input("Title", &self.title)
                    .hint(if self.title_state.is_valid(TITLE_RULES) {
                        "Clear it and leave the field: the error shows"
                    } else {
                        "Not valid yet"
                    })
                    .error(self.title_state.shown_error(TITLE_RULES)),
            )
            .child(
                Field::input("Env file", &self.env)
                    .error(Some("File not found in repo root: .env.local")),
            )
            .child(Field::new(
                "PR provider",
                DropdownButton::new("demo-dropdown", "GitHub").icon(IconName::GitPullRequest),
            ))
            .child(Field::new("What and why", self.body.clone()))
            .child(Field::new(
                "Start from and merge into",
                DropdownButton::new("demo-dropdown-locked", "main")
                    .icon(IconName::GitBranch)
                    .mono()
                    .disabled(true),
            ))
            .child(Field::new(
                "Open",
                DropdownButton::new("demo-dropdown-open", "release/2.3")
                    .mono()
                    .open(true)
                    .on_click(|_, _, _| {}),
            ))
            .child(Field::unlabelled(self.disabled.clone()).hint("Disabled"))
            .into_any_element()
    }

    fn overlays(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let open_primary = cx.entity().downgrade();
        let open_destructive = cx.entity().downgrade();
        div()
            .flex()
            .gap(px(space::S6))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(
                        div()
                            .id("demo-tooltip-target")
                            .child(
                                Button::new("demo-tooltip", "Hover me")
                                    .variant(Variant::Secondary)
                                    .icon(IconName::SquareTerminal),
                            )
                            .tooltip(Tooltip::text("Open terminal", Some("⌘ T".into()))),
                    )
                    .child(self.menu.clone())
                    .child(div().text_size(px(11.)).text_color(c.text_muted).child(
                        SharedString::from(format!(
                                "Chosen: {}",
                                self.chosen
                                    .clone()
                                    .unwrap_or_else(|| "nothing yet (↑/↓, ↵ or click)".into())
                            )),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        Button::new("demo-focus", "Focus the title field")
                            .variant(Variant::Secondary)
                            .on_click({
                                let this = cx.entity().downgrade();
                                move |_, window, cx| {
                                    let _ = this.update(cx, |d, cx| {
                                        let handle = d.title.read(cx).focus_handle(cx);
                                        window.focus(&handle, cx);
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new("demo-dialog", "Open a primary dialog").on_click(
                            move |_, window, cx| {
                                let _ = open_primary.update(cx, |d, cx| {
                                    d.dialog = Some(DialogKind::Primary);
                                    window.focus(&d.dialog_focus, cx);
                                    cx.notify();
                                });
                            },
                        ),
                    )
                    .child(
                        Button::new("demo-dialog-d", "Open a destructive dialog")
                            .variant(Variant::Destructive)
                            .on_click(move |_, window, cx| {
                                let _ = open_destructive.update(cx, |d, cx| {
                                    d.dialog = Some(DialogKind::Destructive);
                                    window.focus(&d.dialog_focus, cx);
                                    cx.notify();
                                });
                            }),
                    ),
            )
            .into_any_element()
    }

    fn dialog(&self, kind: DialogKind, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let close = cx.entity().downgrade();
        let confirm = cx.entity().downgrade();
        let (title, label, icon) = match kind {
            DialogKind::Primary => (
                "Send SHOP-18 Checkout API back?",
                "Send back",
                IconName::Undo2,
            ),
            DialogKind::Destructive => (
                "Delete SHOP-21 Order confirmation email?",
                "Delete",
                IconName::Trash2,
            ),
        };
        ConfirmDialog::new(
            "demo-confirm",
            &self.dialog_focus,
            kind,
            title,
            label,
            move |_, cx| {
                let _ = confirm.update(cx, |d, cx| {
                    d.chosen = Some("dialog confirmed".into());
                    d.dialog = None;
                    cx.notify();
                });
            },
            move |_, cx| {
                let _ = close.update(cx, |d, cx| {
                    d.chosen = Some("dialog cancelled".into());
                    d.dialog = None;
                    cx.notify();
                });
            },
        )
        .confirm_icon(icon)
        .confirm_disabled(false)
        .width(440.)
        .child(
            div()
                .flex()
                .gap(px(8.))
                .px(px(10.))
                .py(px(8.))
                .rounded(px(radius::MD))
                .bg(c.st_working_bg)
                .text_size(px(12.))
                .text_color(c.text_primary)
                .child("SHOP-18 goes straight back to its agent in Dev (Agent Working) and continues in the same session."),
        )
        .text("3 line comments will be bundled into one message for the agent.")
        .into_any_element()
    }
}

/// A titled sheet like the gallery's other sections.
fn sheet(title: &str, t: &Theme, body: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(space::S3))
        .p(px(space::S5))
        .bg(t.colors.bg_panel)
        .border_1()
        .border_color(t.colors.border)
        .rounded(px(radius::DIALOG))
        .child(
            div()
                .text_size(px(13.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(t.colors.text_primary)
                .child(SharedString::from(title.to_owned())),
        )
        .child(body)
        .into_any_element()
}

impl ComponentsDemo {
    /// *04 · Ticket card variants → Variant grid* and *Compact density*, from fixtures.
    fn cards(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::components::card::{ChainState, Density, TicketCard};
        use crate::view_models::card::CardViewModel;
        use crate::view_models::card_fixtures::{Look, NOW, variants};
        let c = ActiveTheme::get(cx).colors;
        let cell = |i: usize, name: &'static str, card: TicketCard| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .w(px(224.))
                .p(px(6.))
                .rounded(px(radius::LG))
                .bg(c.bg_column)
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(10.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(name.to_uppercase())),
                )
                .child(card.width(212.))
                .id(("card-cell", i))
        };
        let grid = variants().into_iter().enumerate().map(|(i, v)| {
            let vm = CardViewModel::from(&v.ticket, &v.extras, NOW);
            let clicked = cx.entity().downgrade();
            let acted = cx.entity().downgrade();
            let opened = cx.entity().downgrade();
            let mut card = TicketCard::new(("card", i), vm)
                .on_click(move |_, _, cx| {
                    let _ = clicked.update(cx, |d, cx| {
                        d.chosen = Some("card clicked".into());
                        cx.notify();
                    });
                })
                .on_action(move |_, _, cx| {
                    let _ = acted.update(cx, |d, cx| {
                        d.chosen = Some("card action".into());
                        cx.notify();
                    });
                })
                .on_open_panel(move |_, _, cx| {
                    let _ = opened.update(cx, |d, cx| {
                        d.chosen = Some("open panel".into());
                        cx.notify();
                    });
                });
            card = match v.look {
                Look::Plain => card,
                Look::Selected => card.selected(true),
                Look::Hovered => card.show_hover(true),
                Look::Dragging => card.dragging(true),
                Look::Chain => card.chain(ChainState::Highlighted),
                Look::Dimmed => card.chain(ChainState::Dimmed),
            };
            cell(i, v.name, card)
        });
        let compact = variants()
            .into_iter()
            .filter(|v| {
                [
                    "Working · needs you",
                    "Working · normal + running",
                    "Pinned",
                ]
                .contains(&v.name)
            })
            .enumerate()
            .map(|(i, v)| {
                TicketCard::new(
                    ("compact", i),
                    CardViewModel::from(&v.ticket, &v.extras, NOW),
                )
                .density(Density::Compact)
                .width(168.)
            });
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(div().flex().flex_wrap().gap(px(12.)).children(grid))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(c.text_secondary)
                    .child("Compact density"),
            )
            .child(div().flex().gap(px(8.)).children(compact))
            .into_any_element()
    }
}

impl ComponentsDemo {
    /// *04 → Row · epics* and *Row · epics as delivery units (0059)*, from fixtures.
    fn epics(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::components::epic_card::EpicCard;
        use crate::view_models::card_fixtures::epic_variants;
        use crate::view_models::epic_card::EpicViewModel;
        let cards = epic_variants().into_iter().enumerate().map(|(i, v)| {
            let vm = EpicViewModel::from(&v.epic, &v.tickets, v.branch);
            let toggle = cx.entity().downgrade();
            let child = cx.entity().downgrade();
            let add = cx.entity().downgrade();
            let click = cx.entity().downgrade();
            let act = cx.entity().downgrade();
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(10.))
                        .text_color(ActiveTheme::get(cx).colors.text_muted)
                        .child(SharedString::from(v.name.to_uppercase())),
                )
                .child(
                    EpicCard::new(("epic", i), vm)
                        .width(212.)
                        .collapsed(self.collapsed.contains(&i))
                        .on_toggle(move |_, _, cx| {
                            let _ = toggle.update(cx, |d, cx| {
                                if let Some(p) = d.collapsed.iter().position(|c| *c == i) {
                                    d.collapsed.remove(p);
                                } else {
                                    d.collapsed.push(i);
                                }
                                cx.notify();
                            });
                        })
                        .on_child(move |_, _, cx| {
                            let _ = child.update(cx, |d, cx| {
                                d.chosen = Some("epic child selected".into());
                                cx.notify();
                            });
                        })
                        .on_add(move |_, _, cx| {
                            let _ = add.update(cx, |d, cx| {
                                d.chosen = Some("add ticket to epic".into());
                                cx.notify();
                            });
                        })
                        .on_click(move |_, _, cx| {
                            let _ = click.update(cx, |d, cx| {
                                d.chosen = Some("epic clicked".into());
                                cx.notify();
                            });
                        })
                        .on_action(move |_, _, cx| {
                            let _ = act.update(cx, |d, cx| {
                                d.chosen = Some("epic action".into());
                                cx.notify();
                            });
                        })
                        .selected(false)
                        .dragging(false)
                        .chain(crate::components::card::ChainState::None),
                )
        });
        div()
            .flex()
            .flex_wrap()
            .gap(px(12.))
            .children(cards)
            .into_any_element()
    }
}

impl ComponentsDemo {
    /// *12 · Stack & dependencies → Stack list · detail*, from fixtures (SHOP-17 → SHOP-18 → SHOP-20 ← SHOP-19).
    fn stack(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::components::stack_list::StackList;
        use crate::view_models::card_fixtures::ticket;
        use crate::view_models::stack::stack_rows;
        let with = |k: &str, title: &str, status, deps: &[&str]| {
            let mut t = ticket(k, title, status);
            t.depends_on = deps.iter().map(|d| d.parse().expect("key")).collect();
            t
        };
        let tickets = vec![
            with("SHOP-17", "Add Stripe client", Status::Merged, &[]),
            with("SHOP-18", "Checkout API", Status::Backlog, &["SHOP-17"]),
            with("SHOP-19", "Price table", Status::Backlog, &[]),
            with(
                "SHOP-20",
                "Checkout page",
                Status::Backlog,
                &["SHOP-18", "SHOP-19"],
            ),
            with(
                "SHOP-21",
                "Order confirmation email",
                Status::Backlog,
                &["SHOP-20"],
            ),
        ];
        let current: crewboard_core::domain::TicketKey = "SHOP-20".parse().expect("key");
        let lone: crewboard_core::domain::TicketKey = "SHOP-12".parse().expect("key");
        let clicked = cx.entity().downgrade();
        div()
            .flex()
            .gap(px(16.))
            .child(div().w(px(340.)).child(
                StackList::new(current.clone(), stack_rows(&current, &tickets)).on_select(
                    move |key, _, cx| {
                        let _ = clicked.update(cx, |d, cx| {
                            d.chosen = Some(format!("stack: {key}").into());
                            cx.notify();
                        });
                    },
                ),
            ))
            .child(div().w(px(340.)).child(StackList::new(lone, Vec::new())))
            .into_any_element()
    }
}

impl Render for ComponentsDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = ActiveTheme::get(cx).clone();
        let _ = &self.title_empty;
        let dialog = self.dialog.map(|k| self.dialog(k, cx));
        div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(space::S5))
            .child(sheet(
                "Badges & Inputs · Badges & pills",
                &t,
                self.badges(&t),
            ))
            .child(sheet(
                "Badges & Inputs · Tabs & segmented",
                &t,
                self.tabs(cx),
            ))
            .child(sheet("Badges & Inputs · Fields", &t, self.fields()))
            .child(sheet(
                "Overlays & feedback · tooltip, context menu, confirm dialog",
                &t,
                self.overlays(cx),
            ))
            .child(sheet(
                "04 · Ticket card variants · Variant grid (fixtures)",
                &t,
                self.cards(cx),
            ))
            .child(sheet(
                "12 · Stack & dependencies · Stack list (fixtures)",
                &t,
                self.stack(cx),
            ))
            .child(sheet(
                "04 · Ticket card variants · Row · epics (fixtures)",
                &t,
                self.epics(cx),
            ))
            .children(dialog)
    }
}

/// Keeps the demo's entity type nameable from the gallery.
pub fn new(window: &mut Window, cx: &mut App) -> Entity<ComponentsDemo> {
    cx.new(|cx| ComponentsDemo::new(window, cx))
}
