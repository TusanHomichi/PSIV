# Shop and inn layout decode

Piata is the Tier-1 shop oracle target. These captures were made from the
retail core with the tapes in `oracle/tapes/23_piata_shop_reach.tape` and
`oracle/tapes/26_piata_inn_reach.tape`; the tapes walk from the live new-game
route rather than teleporting the party to a counter.

## Evidence

| surface | tape mark | state | retail frame | result |
|---|---|---|---:|---|
| shop shell | `shop_open` | `oracle/states/shop_piata_open.json` | 9461 | item shop mode entered |
| shop greeting | `greeting_drawn` | `oracle/states/shop_piata_greeting.json` | 9581 | keeper portrait and greeting |
| buy list | `buy_list` | `oracle/states/shop_piata_buy_list.json` | 10309 | `MONOMATE`, 20 MST |
| buy quantity/confirm | `quantity_screen` | `oracle/states/shop_piata_buy_confirm.json` | 10433 | quantity pane and item confirm |
| sell list | `sell_list` | `oracle/states/shop_piata_sell_list.json` | 10929 | sell mode entered |
| sell selection | `sell_confirm` | `oracle/states/shop_piata_sell_confirm.json` | 11053 | `MONOMATE` selected |
| sell yes/no | `sell_yes` | `oracle/states/shop_piata_sell_yesno.json` | 11173 | 10 MST confirmation |
| inn greeting | `inn_greeting_drawn` | `oracle/states/shop_piata_inn_greeting.json` | 9429 | keeper portrait and greeting |
| inn yes/no | `rest_confirm` | `oracle/states/shop_piata_inn_confirm.json` | 10152 | 10 MST, yes/no |
| post-night | `night_wake` | `oracle/states/shop_piata_inn_night.json` | 10576 | money and return greeting after the night |

The shop route starts at Piata Academy, joins Alys, takes the item-shop door,
and reaches the keeper at map `$001B`, object 0, position `(33,30)`. The inn
route reaches map `$0019`, object 0, position `(41,30)`. In both rooms the
party stands one cell below the keeper and faces up. This matters: the table is
bound to the keeper's object position, not to the `$C` collision tile in front
of it.

The exploratory route tape `oracle/tapes/24_piata_route_probe.tape` is retained
as negative evidence. The offline route planner/navigator stopped on live
object state; it is not used as the authoritative shop route.

## Camera and screen coordinates

The shop windows use the Genesis 320x224 frame: 40x28 cells, 8x8 pixels per
cell. The raw Plane A dump is a 64x32 circular buffer, so the screen decode
must apply the camera offset before rectangle detection.

| capture | camera foreground/background pixels | buffer offset in cells |
|---|---:|---:|
| Piata item shop | `(376,408)` | `(47,51)` |
| Piata inn | `(504,408)` | `(63,51)` |

For screen cell `(x,y)`, the decoded word is the raw word at
`((x + offset_x) mod 64, (y + offset_y) mod 32)`. The renderer constants below
are screen coordinates, never raw buffer coordinates.

## Decoded outer windows

All rectangles are inclusive of their frame and are listed as
`(x_cell, y_cell, width_cells, height_cells)` followed by pixels.

| constant | retail surface | cells | pixels | evidence |
|---|---|---|---|---|
| `MONEY` | meseta strip | `(2,2,13,3)` | `(16,16,104,24)` | camera-remapped decoder |
| `PORTRAIT` | keeper portrait | `(6,7,6,6)` | `(48,56,48,48)` | capture + portrait window geometry |
| `MAIN_MESSAGE` | greeting/result text | `(3,20,34,6)` | `(24,160,272,48)` | camera-remapped decoder |
| `BUY_SELL_MENU` | `buy` / `sell` | `(5,14,8,5)` | `(40,112,64,40)` | camera-remapped decoder |
| `BUY_LIST` | item and price | `(14,6,20,3)` | `(112,48,160,24)` | camera-remapped decoder |
| `BUY_QUANTITY` | quantity/yes-no pane | `(6,13,7,5)` | `(48,104,56,40)` | reachable capture; special cursor chrome |
| `SELL_ITEM` | selected inventory item | `(18,2,14,3)` | `(144,16,112,24)` | camera-remapped decoder |
| `SELL_PANE` | inventory cursor/yes-no | `(7,13,7,5)` | `(56,104,56,40)` | reachable capture; special cursor chrome |
| `INN_CONFIRM` | stay yes/no | `(6,14,7,5)` | `(48,112,56,40)` | camera-remapped decoder |

The fixed text anchors used by the shop renderer are:

| text | cell origin |
|---|---:|
| money value (`500 MST`-shaped) | `(4,3)` |
| `buy` | `(8,15)` |
| `sell` | `(8,17)` |
| buy item name (`MONOMATE`) | `(17,7)` |
| sell item name (`MONOMATE`) | `(21,3)` |
| inn `YES` | `(9,15)` |
| inn `NO` | `(9,17)` |
| main message lines | `(4,21)` and `(4,22)` |

The buy quantity and sell panes use the same five-row framed geometry but
carry cursor/list words that the generic rectangle matcher does not classify.
They are therefore measured from the reachable captures and are explicitly
covered by the shop module's constants rather than silently inferred from the
camp menu.

The shop table's portrait pointers are also decoded, but the current runtime
pack promotes only the ordinary dialogue portraits. The seven separate
shopkeeper art blobs are still source/reference assets, so the new renderer
draws the decoded six-cell portrait slot and logs the missing promoted PNG
instead of substituting a character portrait. Baker is the one shared art
pointer already available in the dialogue pack.

## Decoder delta and provisional surfaces

The existing `oracle/decode_layout.py` and `oracle/host` were run read-only.
The decoder's generic self-check passes for all ten state files, but its output
is not a shop layout contract:

* it assumes the raw top-left 40x28 words are already the visible screen;
* `_rectangle_kind` contains battle-specific rectangle knowledge;
* every shop output is consequently labelled `psiv_battle_layout`, and most
  shop text/rectangles disappear until the camera wrap is applied;
* number tiles, cursor chrome, and portrait art are not decoded as shop
  semantic fields.

The `oracle/layouts/shop_piata_*.json` files preserve those unmodified tool
outputs. This document records the camera-remapped evidence needed by the
implementation and exposes the tool gap instead of changing the oracle.

All requested normal Tier-1 screens were reachable. The affordability refusal,
inventory-full refusal, and insufficient-inn-funds refusal were not reached on
the Piata tapes; their text/result state is **PROVISIONAL** and is implemented
from the decoded disassembly rules in `docs/SHOPS.md` and `ps4.asm` until a
dedicated refusal tape is authorized. Vehicle recovery and the eight skill-use
fields are also **PROVISIONAL/UNMODELED** in the current runtime state shape;
the runtime seam reports that boundary rather than pretending those fields
exist.
