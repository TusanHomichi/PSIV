//! Retail shop and inn windows.
//!
//! The rectangles here are the camera-remapped Piata decode in
//! `docs/SHOP_LAYOUT_DECODED.md`. The window owns cursors and text only;
//! transaction mutations go through `psiv_runtime::Runtime`.

#[path = "shop/portraits.rs"]
mod portraits;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use godot::classes::{INode2D, Image, ImageTexture, Input, Node2D};
use godot::global::Key;
use godot::prelude::*;
use serde::Deserialize;

use psiv_data::{DialogueSet, Role};
use psiv_runtime::{InnResult, Runtime, ShopBuyResult, ShopSellResult};

const CELL: f32 = 8.0;
const MONEY: CellRect = CellRect::new(2, 2, 13, 3);
const PORTRAIT: CellRect = CellRect::new(6, 7, 6, 6);
const MAIN_MESSAGE: CellRect = CellRect::new(3, 20, 34, 6);
const BUY_SELL_MENU: CellRect = CellRect::new(5, 14, 8, 5);
const BUY_LIST: CellRect = CellRect::new(14, 6, 20, 3);
const BUY_QUANTITY: CellRect = CellRect::new(6, 13, 7, 5);
const SELL_ITEM: CellRect = CellRect::new(18, 2, 14, 3);
const SELL_PANE: CellRect = CellRect::new(7, 13, 7, 5);
const INN_CONFIRM: CellRect = CellRect::new(6, 14, 7, 5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl CellRect {
    const fn new(x: i32, y: i32, w: i32, h: i32) -> CellRect {
        CellRect { x, y, w, h }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Closed,
    Greeting,
    InnGreeting,
    Root,
    BuyList,
    BuyConfirm,
    SellList,
    SellConfirm,
    InnConfirm,
    Message,
}

#[derive(Clone, Debug, Default)]
struct Snapshot {
    money: u32,
    party_slots: usize,
    items: Vec<OwnedItem>,
}

#[derive(Clone, Debug)]
struct OwnedItem {
    slot: usize,
    id: u8,
    name: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ShopsFile {
    #[serde(default)]
    portraits: Vec<portraits::Portrait>,
    counters: Vec<ShopCounter>,
    inventories: Vec<ShopInventory>,
    inns: Vec<InnRecord>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ShopCounter {
    id: usize,
    #[serde(default = "default_live")]
    live: bool,
    map_id: u16,
    kind: String,
    x_cell: u16,
    y_cell: u16,
    #[serde(default)]
    inn_index: Option<usize>,
    #[serde(default)]
    shop_inventory_index: Option<usize>,
    portrait: String,
    #[serde(default)]
    greeting: Option<GreetingSelector>,
}

impl ShopCounter {
    fn is_inn(&self) -> bool {
        self.kind == "inn"
    }
}

fn default_live() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
struct GreetingSelector {
    #[serde(default)]
    trade_fragment: u8,
}

#[derive(Clone, Debug, Deserialize)]
struct ShopInventory {
    index: usize,
    items: Vec<ShopItem>,
}

#[derive(Clone, Debug, Deserialize)]
struct ShopItem {
    item_id: u8,
    buy_price: u32,
    display_name: String,
    symbol: String,
}

#[derive(Clone, Debug, Deserialize)]
struct InnRecord {
    index: usize,
    rate_per_character: u32,
}

#[derive(Clone, Debug)]
struct ShopCatalog {
    portraits: HashMap<String, String>,
    counters: Vec<ShopCounter>,
    inventories: Vec<ShopInventory>,
    inns: Vec<InnRecord>,
    names: HashMap<u8, String>,
}

impl ShopCatalog {
    fn load(pack_dir: &str) -> Result<ShopCatalog, String> {
        let path = Path::new(pack_dir).join("shops.json");
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let file: ShopsFile = serde_json::from_str(&text)
            .map_err(|error| format!("could not decode {}: {error}", path.display()))?;
        let mut names = HashMap::new();
        for inventory in &file.inventories {
            for item in &inventory.items {
                names
                    .entry(item.item_id)
                    .or_insert_with(|| item.display_name.clone());
            }
        }
        Ok(ShopCatalog {
            portraits: file.portraits.into_iter().map(|p| (p.art, p.png)).collect(),
            counters: file.counters,
            inventories: file.inventories,
            inns: file.inns,
            names,
        })
    }

    fn counter_at(&self, map_id: u16, x: u16, y: u16) -> Option<ShopCounter> {
        self.counters
            .iter()
            .find(|counter| {
                counter.live
                    && counter.map_id == map_id
                    && counter.x_cell == x
                    && counter.y_cell == y
            })
            .cloned()
    }

    fn counter_index(&self, index: usize) -> Option<ShopCounter> {
        self.counters
            .iter()
            .find(|counter| counter.live && counter.id == index)
            .or_else(|| self.counters.get(index))
            .filter(|counter| counter.live)
            .cloned()
    }

    fn inventory(&self, index: usize) -> Option<&ShopInventory> {
        self.inventories
            .iter()
            .find(|inventory| inventory.index == index)
    }

    fn inn(&self, index: usize) -> Option<&InnRecord> {
        self.inns.iter().find(|inn| inn.index == index)
    }

    fn item_name(&self, item: u8) -> String {
        self.names
            .get(&item)
            .cloned()
            .unwrap_or_else(|| format!("ITEM {item}"))
    }
}

struct Quad {
    texture: Gd<ImageTexture>,
    dest: Rect2,
    src: Rect2,
}

type ShopDrawList = (Vec<Quad>, Option<(Gd<ImageTexture>, Rect2)>);

struct ShopChrome {
    tiles: BTreeMap<&'static str, Gd<ImageTexture>>,
    words: BTreeMap<u16, Gd<ImageTexture>>,
    font: Gd<ImageTexture>,
    glyph_at: BTreeMap<char, Vector2>,
}

impl ShopChrome {
    fn build(pack_dir: &str, set: &DialogueSet) -> Option<ShopChrome> {
        let strip = load_image(pack_dir, &set.window.png)?;
        let mut tiles = BTreeMap::new();
        for role in Role::ALL {
            let tile = set.window.role(role)?;
            let region = Rect2i::new(
                Vector2i::new(tile.x, tile.y),
                Vector2i::new(tile.width as i32, tile.height as i32),
            );
            let mut cell = strip.get_region(region)?;
            if tile.flip_h {
                cell.flip_x();
            }
            if tile.flip_v {
                cell.flip_y();
            }
            tiles.insert(role.as_str(), ImageTexture::create_from_image(&cell)?);
        }
        let menu_path = format!("{pack_dir}/dialogue/menu_font.png");
        let menu = Image::load_from_file(&GString::from(menu_path.as_str())).or_else(|| {
            let path = Path::new(pack_dir)
                .parent()?
                .join("generated/gfx/ArtNem_Font.png");
            Image::load_from_file(&GString::from(path.to_string_lossy().as_ref()))
        })?;
        let font = ImageTexture::create_from_image(&menu)?;
        let mut words = BTreeMap::new();
        for pattern in [0x680, 0x6E7, 0x6E8] {
            let source = Rect2i::new(
                Vector2i::new(i32::from(pattern - 0x680) * 8, 0),
                Vector2i::new(8, 8),
            );
            words.insert(
                pattern,
                ImageTexture::create_from_image(&strip.get_region(source)?)?,
            );
        }
        Some(ShopChrome {
            tiles,
            words,
            font,
            glyph_at: retail_glyphs(),
        })
    }

    fn frame(&self, rect: CellRect) -> Vec<Quad> {
        let mut quads = Vec::new();
        let mut push = |name: &'static str, x: i32, y: i32| {
            if let Some(texture) = self.tiles.get(name) {
                quads.push(Quad {
                    texture: texture.clone(),
                    dest: Rect2::new(
                        Vector2::new(x as f32 * CELL, y as f32 * CELL),
                        Vector2::new(CELL, CELL),
                    ),
                    src: Rect2::new(Vector2::ZERO, Vector2::new(CELL, CELL)),
                });
            }
        };
        for y in 1..rect.h - 1 {
            for x in 1..rect.w - 1 {
                push("fill", rect.x + x, rect.y + y);
            }
        }
        for x in 1..rect.w - 1 {
            push("edge_top", rect.x + x, rect.y);
            push("edge_bottom", rect.x + x, rect.y + rect.h - 1);
        }
        for y in 1..rect.h - 1 {
            push("edge_left", rect.x, rect.y + y);
            push("edge_right", rect.x + rect.w - 1, rect.y + y);
        }
        push("corner_top_left", rect.x, rect.y);
        push("corner_top_right", rect.x + rect.w - 1, rect.y);
        push("corner_bottom_left", rect.x, rect.y + rect.h - 1);
        push(
            "corner_bottom_right",
            rect.x + rect.w - 1,
            rect.y + rect.h - 1,
        );
        quads
    }

    fn word(&self, pattern: u16, cell: (i32, i32)) -> Option<Quad> {
        Some(Quad {
            texture: self.words.get(&pattern)?.clone(),
            dest: Rect2::new(
                Vector2::new(cell.0 as f32 * CELL, cell.1 as f32 * CELL),
                Vector2::new(CELL, CELL),
            ),
            src: Rect2::new(Vector2::ZERO, Vector2::new(CELL, CELL)),
        })
    }

    fn text(&self, text: &str, cell: (i32, i32)) -> Vec<Quad> {
        let mut quads = Vec::new();
        for (row, line) in text.lines().enumerate() {
            for (column, character) in line.chars().enumerate() {
                let dest_cell = (cell.0 + column as i32, cell.1 + row as i32);
                if character == ' ' {
                    if let Some(quad) = self.word(0x680, dest_cell) {
                        quads.push(quad);
                    }
                    continue;
                }
                let Some(source) = self
                    .glyph_at
                    .get(&character)
                    .or_else(|| self.glyph_at.get(&'?'))
                else {
                    continue;
                };
                quads.push(Quad {
                    texture: self.font.clone(),
                    dest: Rect2::new(
                        Vector2::new(dest_cell.0 as f32 * CELL, dest_cell.1 as f32 * CELL),
                        Vector2::new(CELL, CELL),
                    ),
                    src: Rect2::new(*source, Vector2::new(CELL, CELL)),
                });
            }
        }
        quads
    }
}

/// A modal shop/inn window owned by the field shell.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct ShopWindow {
    base: Base<Node2D>,
    pack_dir: String,
    chrome: Option<ShopChrome>,
    catalog: Option<ShopCatalog>,
    counter: Option<ShopCounter>,
    portrait: Option<Gd<ImageTexture>>,
    mode: Mode,
    root_selection: usize,
    item_selection: usize,
    confirm_selection: usize,
    snapshot: Snapshot,
    message: String,
    accept_blocked: bool,
    cancel_down: bool,
}

#[godot_api]
impl INode2D for ShopWindow {
    fn init(base: Base<Node2D>) -> Self {
        ShopWindow {
            base,
            pack_dir: String::new(),
            chrome: None,
            catalog: None,
            counter: None,
            portrait: None,
            mode: Mode::Closed,
            root_selection: 0,
            item_selection: 0,
            confirm_selection: 0,
            snapshot: Snapshot::default(),
            message: String::new(),
            accept_blocked: false,
            cancel_down: false,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(1000);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
    }

    fn draw(&mut self) {
        let Some(list) = self.draw_list() else {
            return;
        };
        for quad in list.0 {
            self.base_mut()
                .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
        }
        if let Some((portrait, rect)) = list.1 {
            self.base_mut().draw_texture_rect(&portrait, rect, true);
        }
    }
}

impl ShopWindow {
    /// Loads the shared window art and the shop tables.
    pub(crate) fn configure(&mut self, pack_dir: &str) {
        self.pack_dir = pack_dir.to_owned();
        match DialogueSet::load(Path::new(pack_dir)) {
            Ok(set) => {
                self.chrome = ShopChrome::build(pack_dir, &set);
                if self.chrome.is_none() {
                    godot_error!("shop: window/font art failed to load from {pack_dir}");
                }
            }
            Err(error) => godot_error!("shop: dialogue art failed to load: {error}"),
        }
        match ShopCatalog::load(pack_dir) {
            Ok(catalog) => self.catalog = Some(catalog),
            Err(error) => godot_error!("shop: {error}"),
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.mode != Mode::Closed
    }

    pub(crate) fn debug_menu(&self) -> serde_json::Value {
        serde_json::json!({
            "mode": format!("{:?}", self.mode),
            "root": self.root_selection,
            "item": self.item_selection,
            "confirm": self.confirm_selection,
            "stock": self.buy_items().iter().map(|item| serde_json::json!({
                "id": item.item_id, "name": item.display_name, "price": item.buy_price,
            })).collect::<Vec<_>>(),
            "message": self.message,
            "money": self.snapshot.money,
        })
    }

    pub(crate) fn counter_at(&self, map_id: u16, x: u16, y: u16) -> Option<ShopCounter> {
        self.catalog.as_ref()?.counter_at(map_id, x, y)
    }

    pub(crate) fn open_index(&mut self, index: usize, runtime: &Runtime) -> bool {
        let Some(counter) = self.catalog.as_ref().and_then(|c| c.counter_index(index)) else {
            godot_error!("shop debug selector {index} did not name a live counter");
            return false;
        };
        self.open(counter, runtime)
    }

    pub(crate) fn open(&mut self, counter: ShopCounter, runtime: &Runtime) -> bool {
        self.root_selection = 0;
        self.item_selection = 0;
        self.confirm_selection = 0;
        self.message.clear();
        self.snapshot = self.snapshot(runtime);
        self.portrait = self.load_portrait(&counter.portrait);
        self.mode = if counter.is_inn() {
            Mode::InnGreeting
        } else {
            Mode::Greeting
        };
        self.accept_blocked = true;
        self.counter = Some(counter);
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
        true
    }

    pub(crate) fn handle_input(&mut self, runtime: &mut Runtime) {
        if self.cancel_requested() {
            if self.go_back() {
                self.close();
                return;
            }
            self.snapshot = self.snapshot(runtime);
            self.base_mut().queue_redraw();
            return;
        }
        let input = Input::singleton();
        if self.accept_blocked {
            if !input.is_action_pressed("ui_accept") {
                self.accept_blocked = false;
            }
            self.snapshot = self.snapshot(runtime);
            return;
        }
        let up = input.is_action_just_pressed("ui_up");
        let down = input.is_action_just_pressed("ui_down");
        let accept = input.is_action_just_pressed("ui_accept");
        match self.mode {
            Mode::Greeting => {
                if accept {
                    self.mode = Mode::Root;
                }
            }
            Mode::InnGreeting => {
                if accept {
                    self.mode = Mode::InnConfirm;
                    self.confirm_selection = 0;
                }
            }
            Mode::Root => {
                if up {
                    self.root_selection = wrap(self.root_selection, 2, false);
                } else if down {
                    self.root_selection = wrap(self.root_selection, 2, true);
                } else if accept {
                    if self.root_selection == 0 {
                        self.mode = Mode::BuyList;
                    } else if self.snapshot.items.is_empty() {
                        self.message = "You don't have anything to sell.".to_owned();
                        self.mode = Mode::Message;
                    } else {
                        self.item_selection = 0;
                        self.mode = Mode::SellList;
                    }
                }
            }
            Mode::BuyList => {
                let count = self.buy_items().len();
                if up {
                    self.item_selection = wrap(self.item_selection, count, false);
                } else if down {
                    self.item_selection = wrap(self.item_selection, count, true);
                } else if accept && count > 0 {
                    self.confirm_selection = 0;
                    self.mode = Mode::BuyConfirm;
                }
            }
            Mode::BuyConfirm => {
                if up || down {
                    self.confirm_selection = 1usize.saturating_sub(self.confirm_selection);
                } else if accept {
                    if self.confirm_selection == 0 {
                        self.confirm_buy(runtime);
                    } else {
                        self.mode = Mode::BuyList;
                    }
                }
            }
            Mode::SellList => {
                let count = self.snapshot.items.len();
                if up {
                    self.item_selection = wrap(self.item_selection, count, false);
                } else if down {
                    self.item_selection = wrap(self.item_selection, count, true);
                } else if accept && count > 0 {
                    self.confirm_selection = 0;
                    self.mode = Mode::SellConfirm;
                }
            }
            Mode::SellConfirm => {
                if up || down {
                    self.confirm_selection = 1usize.saturating_sub(self.confirm_selection);
                } else if accept {
                    if self.confirm_selection == 0 {
                        self.confirm_sell(runtime);
                    } else {
                        self.mode = Mode::SellList;
                    }
                }
            }
            Mode::InnConfirm => {
                if up || down {
                    self.confirm_selection = 1usize.saturating_sub(self.confirm_selection);
                } else if accept {
                    if self.confirm_selection == 0 {
                        self.confirm_inn(runtime);
                    } else {
                        self.mode = Mode::InnGreeting;
                    }
                }
            }
            Mode::Message => {
                if accept {
                    self.mode = if self.counter.as_ref().is_some_and(ShopCounter::is_inn) {
                        Mode::InnGreeting
                    } else {
                        Mode::Root
                    };
                }
            }
            Mode::Closed => {}
        }
        self.snapshot = self.snapshot(runtime);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn close(&mut self) {
        self.mode = Mode::Closed;
        self.counter = None;
        self.portrait = None;
        self.message.clear();
        self.base_mut().set_visible(false);
        self.base_mut().queue_redraw();
    }

    fn cancel_requested(&mut self) -> bool {
        let input = Input::singleton();
        let x = input.is_physical_key_pressed(Key::X);
        let edge = x && !self.cancel_down;
        self.cancel_down = x;
        input.is_action_just_pressed("ui_cancel") || edge
    }

    fn go_back(&mut self) -> bool {
        match self.mode {
            Mode::Closed | Mode::Greeting | Mode::InnGreeting | Mode::Root => true,
            Mode::BuyList | Mode::SellList => {
                self.mode = Mode::Root;
                false
            }
            Mode::BuyConfirm => {
                self.mode = Mode::BuyList;
                false
            }
            Mode::SellConfirm => {
                self.mode = Mode::SellList;
                false
            }
            Mode::InnConfirm => {
                self.mode = Mode::InnGreeting;
                false
            }
            Mode::Message => {
                self.mode = if self.counter.as_ref().is_some_and(ShopCounter::is_inn) {
                    Mode::InnGreeting
                } else {
                    Mode::Root
                };
                false
            }
        }
    }

    fn confirm_buy(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.buy_items().get(self.item_selection).cloned() else {
            self.mode = Mode::BuyList;
            return;
        };
        self.message = match runtime.shop_buy(item.item_id, item.buy_price) {
            ShopBuyResult::Bought { .. } => "Thank you very much.\nAnything else?".to_owned(),
            ShopBuyResult::InventoryFull { .. } => "You can't carry anything else.".to_owned(),
            ShopBuyResult::InsufficientFunds { .. } => "You don't have enough money!".to_owned(),
        };
        self.mode = Mode::Message;
    }

    fn confirm_sell(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.snapshot.items.get(self.item_selection) else {
            self.mode = Mode::SellList;
            return;
        };
        let buy_price = self
            .catalog
            .as_ref()
            .and_then(|catalog| {
                catalog
                    .inventories
                    .iter()
                    .flat_map(|i| &i.items)
                    .find(|candidate| candidate.item_id == item.id)
            })
            .map_or(0, |candidate| candidate.buy_price);
        self.message = match runtime.shop_sell(item.slot, buy_price) {
            ShopSellResult::Sold { .. } => "Thank you very much.\nAnything else?".to_owned(),
            ShopSellResult::EmptySlot => "That item is no longer there.".to_owned(),
        };
        self.mode = Mode::Message;
    }

    fn confirm_inn(&mut self, runtime: &mut Runtime) {
        let Some(counter) = self.counter.as_ref() else {
            return;
        };
        let Some(index) = counter.inn_index else {
            self.mode = Mode::InnGreeting;
            return;
        };
        let Some(rate) = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.inn(index))
            .map(|inn| inn.rate_per_character)
        else {
            self.mode = Mode::InnGreeting;
            return;
        };
        self.message = match runtime.shop_stay(rate, index) {
            InnResult::Stayed { .. } => "Thank you very much.\nPlease come again.".to_owned(),
            InnResult::InsufficientFunds { .. } => "You don't have enough money!".to_owned(),
            InnResult::AiedoEventPending { .. } => "Aiedo rest event pending.".to_owned(),
        };
        self.mode = Mode::Message;
    }

    fn snapshot(&self, runtime: &Runtime) -> Snapshot {
        let inventory = runtime.game().inventory();
        let names = self.catalog.as_ref();
        Snapshot {
            money: runtime.game().money(),
            party_slots: runtime.game().party_len(),
            items: inventory
                .slots()
                .iter()
                .enumerate()
                .filter(|&(_, &id)| id != 0)
                .map(|(slot, &id)| OwnedItem {
                    slot,
                    id,
                    name: names.map_or_else(|| format!("ITEM {id}"), |c| c.item_name(id)),
                })
                .collect(),
        }
    }

    fn buy_items(&self) -> Vec<ShopItem> {
        let Some(counter) = self.counter.as_ref() else {
            return Vec::new();
        };
        let Some(index) = counter.shop_inventory_index else {
            return Vec::new();
        };
        self.catalog
            .as_ref()
            .and_then(|catalog| catalog.inventory(index))
            .map_or_else(Vec::new, |inventory| inventory.items.clone())
    }

    fn load_portrait(&self, raw: &str) -> Option<Gd<ImageTexture>> {
        let relative = self
            .catalog
            .as_ref()?
            .portraits
            .get(raw)
            .map(String::as_str)
            .or_else(|| (raw == "0x29FC66").then_some("dialogue/portraits/12_Baker.png"))?;
        let path = Path::new(&self.pack_dir).join(relative);
        Image::load_from_file(&GString::from(path.to_string_lossy().as_ref()))
            .and_then(|image| ImageTexture::create_from_image(&image))
    }

    fn draw_list(&self) -> Option<ShopDrawList> {
        let chrome = self.chrome.as_ref()?;
        let counter = self.counter.as_ref()?;
        let mut quads = Vec::new();
        let mut portrait = None;
        let money = format!("{} MST", self.snapshot.money);
        quads.extend(chrome.frame(MONEY));
        draw_text(chrome, &mut quads, &money, centered_money_cell(&money));

        if counter.is_inn() {
            quads.extend(chrome.frame(PORTRAIT));
            if let Some(texture) = self.portrait.as_ref() {
                portrait = Some((
                    texture.clone(),
                    Rect2::new(Vector2::new(48.0, 56.0), Vector2::new(48.0, 48.0)),
                ));
            }
            match self.mode {
                Mode::InnConfirm => self.draw_inn_confirm(chrome, &mut quads),
                Mode::Message => draw_message(chrome, &mut quads, &self.message),
                _ => draw_message(chrome, &mut quads, "Welcome! This is the inn."),
            }
        } else {
            quads.extend(chrome.frame(BUY_SELL_MENU));
            draw_text(chrome, &mut quads, "buy", (8, 15));
            draw_text(chrome, &mut quads, "sell", (8, 17));
            let cursor_y = 15 + (self.root_selection as i32 * 2);
            if let Some(cursor) = chrome.word(0x6E8, (6, cursor_y)) {
                quads.push(cursor);
            }
            match self.mode {
                Mode::Greeting => draw_message(
                    chrome,
                    &mut quads,
                    &format!("Welcome! This is\nthe {} store.", trade_name(counter)),
                ),
                Mode::BuyList => self.draw_buy_list(chrome, &mut quads, false),
                Mode::BuyConfirm => self.draw_buy_list(chrome, &mut quads, true),
                Mode::SellList => self.draw_sell_list(chrome, &mut quads, false),
                Mode::SellConfirm => self.draw_sell_list(chrome, &mut quads, true),
                Mode::Message => draw_message(chrome, &mut quads, &self.message),
                _ => draw_message(chrome, &mut quads, "Do you need anything else?"),
            }
        }
        Some((quads, portrait))
    }

    fn draw_buy_list(&self, chrome: &ShopChrome, quads: &mut Vec<Quad>, confirm: bool) {
        let items = self.buy_items();
        quads.extend(chrome.frame(BUY_LIST));
        if let Some(item) = items.get(self.item_selection) {
            draw_text(chrome, quads, &item.display_name, (17, 7));
            let price = item.buy_price.to_string();
            draw_text(
                chrome,
                quads,
                &price,
                (32 - price.chars().count() as i32, 7),
            );
            let description = if item.symbol == "Monomate" {
                "A weak medicine that\nrestores HP."
            } else {
                &item.display_name
            };
            if confirm {
                quads.extend(chrome.frame(BUY_QUANTITY));
                draw_yes_no(chrome, quads, self.confirm_selection, (9, 15));
                draw_message(
                    chrome,
                    quads,
                    &format!("{}.\nIs this what you want?", item.display_name),
                );
            } else {
                draw_message(chrome, quads, description);
            }
        }
    }

    fn draw_sell_list(&self, chrome: &ShopChrome, quads: &mut Vec<Quad>, confirm: bool) {
        let Some(item) = self.snapshot.items.get(self.item_selection) else {
            draw_message(chrome, quads, "You don't have anything to sell.");
            return;
        };
        quads.extend(chrome.frame(SELL_ITEM));
        draw_text(chrome, quads, &item.name, (21, 3));
        quads.extend(chrome.frame(SELL_PANE));
        if confirm {
            draw_yes_no(chrome, quads, self.confirm_selection, (10, 15));
            let price = self.sell_price(item.id);
            draw_message(chrome, quads, &format!("{}?\n{} meseta.", item.name, price));
        } else {
            if let Some(cursor) = chrome.word(0x6E8, (8, 14)) {
                quads.push(cursor);
            }
            draw_message(chrome, quads, "What would you like to sell?");
        }
    }

    fn draw_inn_confirm(&self, chrome: &ShopChrome, quads: &mut Vec<Quad>) {
        quads.extend(chrome.frame(INN_CONFIRM));
        draw_yes_no(chrome, quads, self.confirm_selection, (9, 15));
        let cost = self
            .counter
            .as_ref()
            .and_then(|counter| counter.inn_index)
            .and_then(|index| self.catalog.as_ref().and_then(|catalog| catalog.inn(index)))
            .map_or(0, |inn| {
                inn.rate_per_character
                    .saturating_mul(self.snapshot.party_slots as u32)
            });
        draw_message(
            chrome,
            quads,
            &format!("{} meseta.\nWould you care to stay?", cost),
        );
    }

    fn sell_price(&self, item: u8) -> u32 {
        self.catalog
            .as_ref()
            .into_iter()
            .flat_map(|catalog| catalog.inventories.iter())
            .flat_map(|inventory| inventory.items.iter())
            .find(|candidate| candidate.item_id == item)
            .map_or(0, |candidate| candidate.buy_price / 2)
    }
}

fn draw_yes_no(chrome: &ShopChrome, quads: &mut Vec<Quad>, selected: usize, text_cell: (i32, i32)) {
    draw_text(chrome, quads, "YES", text_cell);
    draw_text(chrome, quads, "NO", (text_cell.0, text_cell.1 + 2));
    if let Some(cursor) = chrome.word(0x6E8, (text_cell.0 - 2, text_cell.1 + selected as i32 * 2)) {
        quads.push(cursor);
    }
}

fn draw_message(chrome: &ShopChrome, quads: &mut Vec<Quad>, message: &str) {
    quads.extend(chrome.frame(MAIN_MESSAGE));
    draw_text(chrome, quads, message, (4, 21));
}

fn draw_text(chrome: &ShopChrome, quads: &mut Vec<Quad>, text: &str, cell: (i32, i32)) {
    quads.extend(chrome.text(text, cell));
}

fn centered_money_cell(text: &str) -> (i32, i32) {
    let width = text.chars().count() as i32;
    (2 + ((13 - width).max(0) / 2), 3)
}

fn trade_name(counter: &ShopCounter) -> &'static str {
    match counter.greeting.as_ref().map_or(2, |g| g.trade_fragment) {
        0 => "weapon",
        1 => "armor",
        _ => "item",
    }
}

fn wrap(current: usize, count: usize, forward: bool) -> usize {
    if count == 0 {
        return 0;
    }
    if forward {
        (current + 1) % count
    } else if current == 0 {
        count - 1
    } else {
        current - 1
    }
}

fn retail_glyphs() -> BTreeMap<char, Vector2> {
    let mut glyphs = BTreeMap::new();
    for (index, ch) in ('A'..='Z').enumerate() {
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * CELL, (index / 16) as f32 * CELL),
        );
    }
    for (index, ch) in ('0'..='9').enumerate() {
        let index = index + 26;
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * CELL, (index / 16) as f32 * CELL),
        );
    }
    for (index, ch) in ('a'..='z').enumerate() {
        let index = index + 56;
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * CELL, (index / 16) as f32 * CELL),
        );
    }
    for (index, ch) in [
        (48, '-'),
        (49, '!'),
        (50, '?'),
        (51, ':'),
        (52, ','),
        (53, '.'),
        (54, '<'),
        (55, '>'),
    ] {
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * CELL, (index / 16) as f32 * CELL),
        );
    }
    glyphs
}

fn load_image(pack_dir: &str, name: &str) -> Option<Gd<Image>> {
    let path = format!("{pack_dir}/{name}");
    Image::load_from_file(&GString::from(path.as_str()))
}

impl crate::Field {
    pub(crate) fn drive_shop_if_active(&mut self) -> bool {
        if !self.shop.as_ref().is_some_and(|shop| shop.bind().is_open()) {
            return false;
        }
        if let (Some(runtime), Some(shop)) = (self.runtime.as_mut(), self.shop.as_mut()) {
            shop.bind_mut().handle_input(runtime);
        }
        if !self.shop.as_ref().is_some_and(|shop| shop.bind().is_open()) {
            if let Some(runtime) = self.runtime.as_mut() {
                runtime.set_field_suspended(false);
            }
            self.sync_visuals(false);
            return true;
        }
        let events = self
            .runtime
            .as_mut()
            .map(|runtime| {
                runtime.set_field_suspended(true);
                runtime.tick(psiv_core::Input::Neutral)
            })
            .unwrap_or_default();
        self.process_events(events);
        self.place_shop_window();
        self.sync_visuals(false);
        true
    }

    pub(crate) fn place_shop_window(&mut self) {
        let Some(camera) = self.camera.as_ref() else {
            return;
        };
        let center = camera.get_position();
        if let Some(shop) = self.shop.as_mut() {
            shop.set_position(center - Vector2::new(160.0, 112.0));
        }
    }
}
