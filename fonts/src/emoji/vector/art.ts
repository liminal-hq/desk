// The vector emoji: each a function that draws its layers, back to front, with the kit (kit.ts) in a 1000-unit box,
// y down. Flat colour, simple shapes, light from the top left as in the pixel set.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
  arc,
  box,
  type Colour,
  difference,
  disc,
  drop,
  grow,
  heart,
  intersection,
  type Layer,
  line,
  mirror,
  move,
  pie,
  poly,
  rounded,
  type Shape,
  star,
  turn,
  union,
} from './kit';

const L = (colour: Colour, shape: Shape, mono?: Layer['mono']): Layer => (mono ? { colour, shape, mono } : { colour, shape });
/** A shading layer: colour only, no part in the monochrome glyph. */
const S = (colour: Colour, shape: Shape): Layer => L(colour, shape, 'skip');

/** A light object on any background: an edge of `edge` colour under a fill inset by `w`. */
const edged = (shape: Shape, fill: Colour, edge: Colour = 'greyLight', w = 36): Layer[] => [L(edge, shape), L(fill, grow(shape, -w))];

// ---- faces ----

const FACE = disc(500, 500, 440);
/** The face disc with a darker crescent on the lower right, as the pixel faces have. */
const face = (fill: Colour = 'amber', shade: Colour = 'amberDark'): Layer[] => [
  L(fill, FACE, 'fill'),
  S(shade, difference(FACE, disc(470, 468, 432))),
];
const EYE_L = 345;
const EYE_R = 655;
const eyes = (y = 390, rx = 48, ry = 78): Layer => L('ink', union(disc(EYE_L, y, rx, ry), disc(EYE_R, y, rx, ry)));
/** Eyes shut in a happy curve (∩). */
const happyEyes = (y = 400): Layer => L('ink', union(arc(EYE_L, y + 30, 75, 60, 200, 340, 52), arc(EYE_R, y + 30, 75, 60, 200, 340, 52)));
/** Eyes shut in a sad or sleeping curve (∪). */
const closedEyes = (y = 400): Layer => L('ink', union(arc(EYE_L, y - 30, 75, 55, 20, 160, 48), arc(EYE_R, y - 30, 75, 55, 20, 160, 48)));
const smile = (rx = 240, ry = 200, a0 = 25, a1 = 155, y = 520, w = 60): Layer => L('ink', arc(500, y, rx, ry, a0, a1, w));
const frown = (y = 800, rx = 190, ry = 140): Layer => L('ink', arc(500, y, rx, ry, 215, 325, 58));
/** An open grin: the lower half of an ellipse in ink, with a band of teeth along its top. */
function grin(top = 560, rx = 270, ry = 230, teeth = 70, tongue = false): Layer[] {
  const mouth = intersection(disc(500, top, rx, ry), box(0, top, 1000, 1000));
  const out = [L('ink', mouth), L('white', intersection(grow(mouth, -26), box(0, top, 1000, top + teeth)))];
  if (tongue) out.push(L('rose', intersection(grow(mouth, -26), disc(500, top + ry, rx * 0.55, ry * 0.5))));
  return out;
}
const tear = (x: number, y: number, s = 1): Layer => L('cyan', drop(x, y, 55 * s, y + 190 * s));

// ---- shared objects ----

function padlock(open: boolean): Layer[] {
  const shackle = arc(open ? 610 : 500, 400, 190, 210, 180, 360, 92, true);
  const legs = open ? union(line([[800, 400], [800, 470]], 92, true)) : union(line([[310, 400], [310, 470]], 92, true), line([[690, 400], [690, 470]], 92, true));
  const body = box(190, 440, 810, 930, 70);
  return [
    L('grey', union(shackle, legs)),
    L('amber', body),
    S('amberDark', difference(body, box(160, 410, 780, 900, 70))),
    L('ink', union(disc(500, 640, 58), poly([470, 650], [530, 650], [550, 800], [450, 800]))),
  ];
}

function speaker(): Layer[] {
  const cone = poly([120, 360], [270, 360], [470, 170], [470, 830], [270, 640], [120, 640]);
  return [L('grey', rounded(cone, 24)), L('greyLight', rounded(poly([140, 380], [260, 380], [260, 620], [140, 620]), 10))];
}

const roundButton = (colour: Colour): Layer[] => [L(colour, box(80, 80, 920, 920, 150))];

function page(): Layer[] {
  const outline = poly([200, 70], [640, 70], [820, 250], [820, 930], [200, 930]);
  const fold = poly([640, 70], [640, 250], [820, 250]);
  return [...edged(rounded(outline, 20), 'white'), L('greyLight', fold, 'ink')];
}

function arrow(angle: number): Layer[] {
  const a = rounded(union(poly([500, 110], [860, 480], [640, 480], [640, 890], [360, 890], [360, 480], [140, 480])), 30);
  return [L('blue', turn(a, angle, 500, 500))];
}

function circle(colour: Colour, shade: Colour): Layer[] {
  const d = disc(500, 500, 420);
  return [L(colour, d, 'ink'), S(shade, difference(d, disc(475, 470, 410)))];
}

function heartOf(colour: Colour, shade?: Colour): Layer[] {
  const h = heart(80, 130, 920, 900);
  const out = [L(colour, h, 'ink')];
  if (shade) out.push(S(shade, difference(h, move(h, -30, -30))));
  out.push(L('white', turn(union(disc(270, 300, 60, 34)), -40, 270, 300)));
  return out;
}

function clockFace(cx: number, cy: number, r: number): Layer[] {
  return [
    L('white', disc(cx, cy, r)),
    L('ink', union(line([[cx, cy], [cx, cy - r * 0.62]], 46), line([[cx, cy], [cx + r * 0.45, cy + r * 0.12]], 46))),
  ];
}

function cloud(): Shape {
  return union(disc(330, 560, 190), disc(560, 450, 240), disc(760, 600, 170), box(150, 560, 900, 770, 110));
}

// ---- the drawings ----

export const VECTOR: Record<string, () => Layer[]> = {
  // Faces
  '🙂': () => [...face(), eyes(), smile(220, 180, 30, 150)],
  '😀': () => [...face(), eyes(360), ...grin()],
  '😃': () => [...face(), eyes(350, 55, 95), ...grin(540, 280, 250)],
  '😄': () => [...face(), happyEyes(380), ...grin(540, 280, 250)],
  '😁': () => [...face(), happyEyes(380), L('ink', box(230, 540, 770, 780, 110)), L('white', box(262, 572, 738, 748, 80)), L('ink', box(240, 650, 760, 672))],
  '😂': () => [...face(), happyEyes(370), ...grin(530, 270, 240), tear(150, 430), tear(850, 430)],
  '😊': () => [...face(), happyEyes(400), L('roseLight', union(disc(250, 580, 80, 50), disc(750, 580, 80, 50))), smile(230, 190, 30, 150, 540)],
  '😉': () => [...face(), L('ink', union(disc(EYE_L, 390, 48, 78), arc(EYE_R, 420, 75, 50, 200, 340, 52))), smile(240, 200, 25, 155)],
  '😍': () => [...face(), L('rose', union(heart(220, 280, 470, 520), heart(530, 280, 780, 520))), ...grin(580, 250, 200)],
  '😘': () => [...face(), L('ink', union(disc(EYE_L, 390, 48, 78), arc(EYE_R, 420, 75, 50, 200, 340, 52), arc(500, 640, 55, 45, 270, 450, 44), arc(500, 730, 55, 45, 270, 450, 44))), L('rose', heart(660, 620, 860, 810))],
  '😮': () => [...face(), eyes(380), L('ink', disc(500, 700, 95, 125))],
  '😐': () => [...face(), eyes(), L('ink', line([[330, 700], [670, 700]], 62))],
  '😶': () => [...face(), eyes(440, 50, 85)],
  '🙁': () => [...face(), eyes(), frown(810, 200, 150)],
  '😌': () => [...face(), closedEyes(440), smile(200, 160, 30, 150, 560)],
  '😢': () => [...face(), eyes(390), frown(820, 190, 140), tear(300, 520)],
  '😭': () => [...face(), closedEyes(420), L('cyan', union(box(270, 470, 380, 900, 40), box(620, 470, 730, 900, 40))), L('ink', disc(500, 700, 110, 90))],
  '😠': () => [...face(), eyes(430, 46, 70), L('ink', union(line([[250, 300], [420, 360]], 58), line([[750, 300], [580, 360]], 58))), frown(820, 200, 140)],
  '😡': () => [...face('rose', 'roseDark'), eyes(430, 46, 70), L('ink', union(line([[250, 300], [420, 360]], 58), line([[750, 300], [580, 360]], 58))), frown(820, 200, 140)],
  '🥵': () => [
    ...face('rose', 'roseDark'),
    eyes(370, 46, 70),
    L('ink', disc(500, 650, 175, 160)),
    L('roseLight', intersection(grow(disc(500, 650, 175, 160), -30), disc(500, 800, 120, 100)), 'hole'),
    // The sweat sits on the face's edge, where its outline would read as ears in the monochrome glyph.
    S('cyan', drop(120, 250, 55, 440)),
    S('cyan', drop(880, 250, 55, 440)),
  ],
  '🫪': () => [
    // Two big eyes of different sizes and heights, pupils looking different ways, and a small low mouth.
    ...face(),
    // The whites are holes in the monochrome glyph, so the eyes stay rings with pupils rather than filling in.
    L('ink', union(disc(325, 390, 135, 145), disc(665, 430, 115, 120))),
    L('white', grow(union(disc(325, 390, 135, 145), disc(665, 430, 115, 120)), -30), 'hole'),
    L('ink', union(disc(280, 335, 42), disc(710, 490, 40))),
    L('ink', box(440, 700, 575, 740, 20)),
  ],
  '🥶': () => [
    ...face('blue', 'blueDark'),
    L('white', intersection(FACE, union(box(0, 0, 1000, 230), poly([290, 220], [370, 220], [330, 330]), poly([460, 220], [540, 220], [500, 360]), poly([630, 220], [710, 220], [670, 320])))),
    eyes(440, 46, 70),
    L('ink', box(240, 590, 760, 790, 60)),
    L('white', box(274, 624, 726, 756, 34), 'hole'),
    L('ink', union(line([[274, 690], [726, 690]], 26), line([[387, 624], [387, 756]], 24), line([[500, 624], [500, 756]], 24), line([[613, 624], [613, 756]], 24))),
  ],
  '😎': () => [
    ...face(),
    L('ink', union(box(160, 300, 470, 500, 90), box(530, 300, 840, 500, 90), line([[140, 330], [860, 330]], 60))),
    L('inkSoft', union(line([[230, 370], [300, 370]], 30), line([[600, 370], [670, 370]], 30))),
    smile(220, 170, 30, 150, 560),
  ],
  '😴': () => [...face(), closedEyes(460), L('ink', disc(500, 720, 70, 60)), L('blue', union(line([[640, 80], [760, 80], [640, 200], [760, 200]], 38), line([[800, 0], [880, 0], [800, 80], [880, 80]], 30)))],
  '😇': () => [L('cyan', difference(disc(500, 90, 300, 80), disc(500, 90, 230, 40))), ...face().map((l) => ({ ...l, shape: move(l.shape, 0, 60) })), eyes(460), smile(220, 180, 30, 150, 590)],
  '😜': () => [...face(), L('ink', union(disc(EYE_L, 390, 48, 78), arc(EYE_R, 420, 75, 50, 200, 340, 52))), ...grin(560, 260, 230, 50, true)],
  '😛': () => [...face(), eyes(380), ...grin(560, 250, 220, 50, true)],
  '🙃': () => [...face(), eyes(), smile(220, 180, 30, 150)].map((l) => ({ ...l, shape: turn(l.shape, 180, 500, 500) })),
  '👽': () => {
    const head = union(disc(500, 400, 380, 340), poly([170, 520], [830, 520], [560, 930], [440, 930]));
    return [L('green', rounded(head, 60)), S('greenDark', difference(rounded(head, 60), move(rounded(head, 60), -30, -30))), L('ink', union(turn(disc(330, 520, 150, 80), 35, 330, 520), turn(disc(670, 520, 150, 80), -35, 670, 520)), 'hole')];
  },

  // Hearts
  '❤': () => heartOf('rose', 'roseDark'),
  '🧡': () => heartOf('amberDark', 'brownLight'),
  '💛': () => heartOf('amber', 'amberDark'),
  '💚': () => heartOf('green', 'greenDark'),
  '💙': () => heartOf('blue', 'blueDark'),
  '💜': () => heartOf('purple', 'purpleDark'),
  '🖤': () => [L('ink', heart(80, 130, 920, 900)), L('inkSoft', turn(union(disc(270, 300, 60, 34)), -40, 270, 300))],
  '🤍': () => [...edged(heart(80, 130, 920, 900), 'white', 'greyLight', 44)],
  '💔': () => {
    const h = heart(80, 130, 920, 900);
    const crack = line([[500, 230], [440, 400], [560, 540], [470, 700], [520, 900]], 56, true);
    const left = intersection(difference(h, crack), poly([0, 0], [500, 0], [440, 400], [560, 540], [470, 700], [520, 1000], [0, 1000]));
    const right = difference(difference(h, crack), left);
    return [L('rose', union(move(left, -30, 10), move(right, 30, -10)))];
  },

  // Stars, weather and nature
  '⭐': () => [L('amber', star(500, 530, 470, 200, 5, 40)), S('amberDark', difference(star(500, 530, 470, 200, 5, 40), move(star(500, 530, 470, 200, 5, 40), -28, -28)))],
  '✨': () => [L('amber', union(rounded(star(400, 560, 340, 85, 4), 10), rounded(star(780, 220, 170, 45, 4), 6), rounded(star(790, 820, 120, 32, 4), 5)))],
  '⚡': () => [L('amber', rounded(poly([590, 40], [200, 560], [470, 560], [380, 960], [800, 400], [530, 400], [650, 40]), 16))],
  '🔥': () => {
    const outer = union(drop(500, 40, 330, 950), poly([200, 560], [140, 300], [330, 470]), poly([800, 560], [870, 330], [680, 460]));
    return [L('rose', rounded(outer, 30)), L('amber', drop(510, 420, 190, 950))];
  },
  '💧': () => [L('blue', drop(500, 50, 340, 940)), L('cyanLight', turn(disc(380, 640, 50, 95), 25, 380, 640))],
  '❄': () => {
    const arm = union(line([[500, 500], [500, 90]], 64), line([[500, 230], [400, 140]], 54), line([[500, 230], [600, 140]], 54));
    return [L('cyan', union(...[0, 60, 120, 180, 240, 300].map((a) => turn(arm, a, 500, 500))))];
  },
  '☀': () => {
    const ray = union(line([[500, 40], [500, 150]], 70));
    return [L('amber', union(disc(500, 500, 250), ...[0, 45, 90, 135, 180, 225, 270, 315].map((a) => turn(ray, a, 500, 500))))];
  },
  '🌙': () => [L('amber', difference(disc(460, 500, 400), disc(680, 400, 330)))],
  '☁': () => edged(cloud(), 'white', 'greyLight', 40),
  '🌈': () => {
    const band = (r: number, w: number): Shape => arc(500, 820, r, r, 180, 360, w, true);
    return [
      L('rose', band(420, 80)),
      L('amber', band(340, 80), 'hole'),
      L('green', band(260, 80)),
      L('blue', band(180, 80), 'hole'),
      L('purple', band(110, 60)),
    ];
  },
  '🍁': () => {
    const half: [number, number][] = [[500, 60], [575, 215], [665, 165], [635, 380], [805, 265], [780, 350], [925, 345], [840, 485], [895, 530], [660, 650], [690, 730], [530, 680], [530, 800]];
    const pts = [...half, ...[...half].reverse().map(([x, y]) => [1000 - x, y] as [number, number])];
    return [L('rose', union(rounded(poly(...pts), 8), line([[500, 700], [500, 950]], 44))), L('roseDark', union(line([[500, 260], [500, 760]], 26), line([[500, 560], [700, 380]], 22), line([[500, 560], [300, 380]], 22)))];
  },

  // Symbols
  '✔': () => [L('green', line([[170, 520], [400, 760], [840, 230]], 150))],
  '❌': () => [L('rose', union(line([[200, 200], [800, 800]], 150), line([[800, 200], [200, 800]], 150)))],
  '✅': () => [...roundButton('green'), L('white', line([[250, 520], [430, 700], [760, 300]], 110), 'hole')],
  '❎': () => [...roundButton('green'), L('white', union(line([[300, 300], [700, 700]], 110), line([[700, 300], [300, 700]], 110)), 'hole')],
  '⚠': () => {
    const t = rounded(poly([500, 70], [950, 880], [50, 880]), 70);
    return [L('amber', t), L('ink', union(line([[500, 360], [500, 600]], 90), disc(500, 740, 55)))];
  },
  '⛔': () => [L('rose', disc(500, 500, 450)), L('white', box(190, 420, 810, 580, 30), 'hole')],
  '🚫': () => [L('rose', union(difference(disc(500, 500, 440), disc(500, 500, 340)), turn(box(130, 450, 870, 550), 45, 500, 500)))],
  '❓': () => [L('rose', union(arc(500, 330, 200, 190, 180, 405, 120), line([[640, 470], [500, 560], [500, 640]], 120), disc(500, 830, 75)))],
  '❗': () => [L('rose', union(line([[500, 130], [500, 600]], 160), disc(500, 830, 85)))],
  '⬆': () => arrow(0),
  '➡': () => arrow(90),
  '⬇': () => arrow(180),
  '⬅': () => arrow(270),
  '▶': () => [...roundButton('blue'), L('white', rounded(poly([350, 260], [740, 500], [350, 740]), 30), 'hole')],
  '⏸': () => [...roundButton('blue'), L('white', union(box(310, 270, 440, 730, 20), box(560, 270, 690, 730, 20)), 'hole')],
  '⏹': () => [...roundButton('blue'), L('white', box(290, 290, 710, 710, 30), 'hole')],
  '🔴': () => circle('rose', 'roseDark'),
  '🟠': () => circle('amberDark', 'brownLight'),
  '🟡': () => circle('amber', 'amberDark'),
  '🟢': () => circle('green', 'greenDark'),
  '🔵': () => circle('blue', 'blueDark'),
  '🟣': () => circle('purple', 'purpleDark'),
  '➕': () => [L('grey', union(line([[500, 180], [500, 820]], 150), line([[180, 500], [820, 500]], 150)))],
  '➖': () => [L('grey', line([[180, 500], [820, 500]], 150))],
  '💯': () => [
    L('rose', union(line([[150, 200], [150, 620]], 90), arc(420, 410, 120, 210, 0, 360, 90), arc(760, 410, 120, 210, 0, 360, 90))),
    L('rose', union(line([[110, 780], [880, 740]], 70), line([[180, 900], [840, 860]], 60))),
  ],

  // More symbols
  '✖': () => [L('grey', union(line([[230, 230], [770, 770]], 150), line([[770, 230], [230, 770]], 150)))],
  '➗': () => [L('grey', union(line([[180, 500], [820, 500]], 130), disc(500, 250, 90), disc(500, 750, 90)))],
  '‼': () => [L('rose', union(line([[330, 130], [330, 600]], 150), disc(330, 830, 80), line([[670, 130], [670, 600]], 150), disc(670, 830, 80)))],
  '⁉': () => [L('rose', union(line([[260, 130], [260, 600]], 150), disc(260, 830, 80), arc(640, 330, 170, 170, 180, 405, 110), line([[760, 450], [640, 540], [640, 610]], 110), disc(640, 830, 75)))],
  'ℹ': () => [...roundButton('blue'), L('white', union(disc(500, 260, 75), box(430, 400, 570, 780, 30)), 'hole')],
  '♾': () => [L('purple', union(arc(310, 500, 190, 160, 0, 360, 90), arc(690, 500, 190, 160, 0, 360, 90)))],
  '⚫': () => [L('ink', disc(500, 500, 420)), S('inkSoft', difference(disc(500, 500, 420), disc(530, 530, 410)))],
  '⚪': () => edged(disc(500, 500, 420), 'white', 'greyLight', 40),
  '🟤': () => circle('brown', 'brownLight'),
  '⬛': () => [L('ink', box(110, 110, 890, 890, 40))],
  '⬜': () => edged(box(110, 110, 890, 890, 40), 'white', 'greyLight', 40),
  '🔶': () => [L('amberDark', rounded(poly([500, 60], [940, 500], [500, 940], [60, 500]), 40))],
  '🔷': () => [L('blue', rounded(poly([500, 60], [940, 500], [500, 940], [60, 500]), 40))],
  '🔺': () => [L('rose', rounded(poly([500, 170], [880, 830], [120, 830]), 40))],
  '🔻': () => [L('rose', rounded(poly([500, 830], [880, 170], [120, 170]), 40))],
  '♠': () => [L('ink', union(turn(heart(90, 120, 910, 820), 180, 500, 470), poly([500, 600], [690, 940], [310, 940])))],
  '♥': () => [L('rose', heart(90, 160, 910, 880))],
  '♦': () => [L('rose', poly([500, 60], [840, 500], [500, 940], [160, 500]))],
  '♣': () => [L('ink', union(disc(500, 290, 200), disc(290, 590, 200), disc(710, 590, 200), poly([500, 560], [680, 940], [320, 940])))],
  '◀': () => [...roundButton('blue'), L('white', rounded(poly([650, 260], [260, 500], [650, 740]), 30), 'hole')],
  '⏩': () => [...roundButton('blue'), L('white', union(rounded(poly([230, 290], [500, 500], [230, 710]), 20), rounded(poly([500, 290], [770, 500], [500, 710]), 20)), 'hole')],
  '⏪': () => [...roundButton('blue'), L('white', union(rounded(poly([770, 290], [500, 500], [770, 710]), 20), rounded(poly([500, 290], [230, 500], [500, 710]), 20)), 'hole')],
  '⏭': () => [...roundButton('blue'), L('white', union(rounded(poly([220, 290], [450, 500], [220, 710]), 20), rounded(poly([450, 290], [680, 500], [450, 710]), 20), box(690, 290, 770, 710, 10)), 'hole')],
  '⏮': () => [...roundButton('blue'), L('white', union(rounded(poly([780, 290], [550, 500], [780, 710]), 20), rounded(poly([550, 290], [320, 500], [550, 710]), 20), box(230, 290, 310, 710, 10)), 'hole')],
  '🔁': () => [...roundButton('blue'), L('white', union(line([[260, 560], [260, 340], [700, 340]], 80), poly([660, 230], [800, 340], [660, 450]), line([[740, 440], [740, 660], [300, 660]], 80), poly([340, 550], [200, 660], [340, 770])), 'hole')],
  '🔄': () => [...roundButton('blue'), L('white', union(arc(500, 500, 250, 250, 200, 340, 80, true), poly([700, 300], [800, 470], [640, 470]), arc(500, 500, 250, 250, 20, 160, 80, true), poly([300, 700], [200, 530], [360, 530])), 'hole')],
  '🔀': () => [...roundButton('blue'), L('white', union(line([[220, 330], [400, 330], [600, 670], [720, 670]], 70), line([[220, 670], [400, 670], [600, 330], [720, 330]], 70), poly([700, 240], [820, 330], [700, 420]), poly([700, 580], [820, 670], [700, 760])), 'hole')],
  '↩': () => [...roundButton('blue'), L('white', union(arc(560, 520, 200, 160, 270, 450, 80, true), line([[560, 680], [380, 680]], 80, true), poly([200, 360], [380, 230], [380, 490]), line([[380, 360], [560, 360]], 80, true)), 'hole')],
  '↪': () => {
    const l = [...roundButton('blue'), L('white', union(arc(560, 520, 200, 160, 270, 450, 80, true), line([[560, 680], [380, 680]], 80, true), poly([200, 360], [380, 230], [380, 490]), line([[380, 360], [560, 360]], 80, true)), 'hole' as const)];
    return l.map((x) => ({ ...x, shape: mirror(x.shape) }));
  },

  // More feelings, weather and nature
  '💬': () => edged(union(box(80, 140, 920, 700, 160), poly([250, 650], [460, 650], [220, 900])), 'white', 'greyLight', 40),
  '💭': () => edged(union(disc(560, 380, 360, 260), disc(240, 760, 80), disc(120, 900, 50)), 'white', 'greyLight', 40),
  '💤': () => [L('blue', union(line([[160, 520], [460, 520], [160, 880], [460, 880]], 70), line([[560, 280], [780, 280], [560, 540], [780, 540]], 56), line([[800, 80], [930, 80], [800, 230], [930, 230]], 42)))],
  '💥': () => [L('rose', rounded(star(500, 500, 470, 230, 9), 20)), L('amber', rounded(star(500, 500, 290, 140, 9), 14))],
  '🌟': () => [L('amberLight', star(500, 530, 490, 230, 5, 30)), L('amber', star(500, 530, 400, 180, 5, 30))],
  '🌕': () => [L('amber', disc(500, 500, 430)), S('amberDark', union(disc(360, 360, 70), disc(640, 560, 90), disc(420, 700, 55)))],
  '⛅': () => [L('amber', union(disc(640, 360, 220), ...[0, 45, 90, 135, 180, 225, 270, 315].map((a) => turn(line([[640, 70], [640, 110]], 60), a, 640, 360)))), ...edged(move(cloud(), -60, 100), 'white', 'greyLight', 40)],
  '🌧': () => [...edged(move(cloud(), 0, -120), 'white', 'greyLight', 40), L('blue', union(line([[300, 760], [260, 900]], 50), line([[500, 760], [460, 900]], 50), line([[700, 760], [660, 900]], 50)))],
  '☔': () => [L('purple', pie(500, 520, 420, 380, 180, 360)), L('ink', union(line([[500, 160], [500, 820]], 50, true), arc(570, 820, 70, 70, 0, 180, 50))), L('blue', union(line([[180, 60], [150, 160]], 40), line([[820, 60], [790, 160]], 40), line([[180, 640], [150, 740]], 40), line([[820, 640], [790, 740]], 40)))],
  '🌲': () => [L('brown', box(450, 780, 550, 950)), L('green', union(poly([500, 60], [760, 400], [240, 400]), poly([500, 220], [830, 620], [170, 620]), poly([500, 400], [900, 820], [100, 820]))), S('greenDark', poly([500, 400], [900, 820], [700, 820]))],
  '🍀': () => [L('green', union(...[0, 90, 180, 270].map((a) => turn(heart(300, 40, 700, 500), a, 500, 500)), line([[500, 560], [640, 950]], 54))), S('greenDark', union(line([[500, 160], [500, 840]], 24), line([[160, 500], [840, 500]], 24)))],

  // More objects
  '🎈': () => [L('grey', line([[500, 760], [460, 860], [520, 950]], 24)), L('rose', turn(drop(500, 80, 320, 780), 180, 500, 430)), L('roseLight', turn(disc(380, 270, 50, 90), 30, 380, 270))],
  '🎯': () => [L('rose', disc(500, 500, 440)), L('white', disc(500, 500, 330), 'hole'), L('rose', disc(500, 500, 220)), L('white', disc(500, 500, 110), 'hole'), L('rose', disc(500, 500, 50))],
  '🎲': () => [...edged(box(110, 110, 890, 890, 120), 'white', 'greyLight', 40), L('ink', union(disc(300, 300, 70), disc(500, 500, 70), disc(700, 700, 70), disc(700, 300, 70), disc(300, 700, 70)))],
  '💎': () => [L('cyan', poly([200, 200], [800, 200], [960, 400], [500, 920], [40, 400])), L('cyanLight', poly([200, 200], [500, 200], [360, 400], [40, 400]), 'skip'), L('blue', poly([500, 920], [960, 400], [640, 400]), 'skip')],
  '🔋': () => [L('grey', union(box(80, 260, 860, 740, 60), box(840, 400, 940, 600, 20))), L('inkSoft', box(140, 320, 800, 680, 30)), L('green', box(180, 360, 620, 640, 20))],
  '🔍': () => [L('grey', line([[620, 620], [880, 880]], 120)), L('grey', disc(400, 400, 300)), L('cyanLight', disc(400, 400, 220), 'hole')],
  '📱': () => [L('inkSoft', box(260, 60, 740, 940, 70)), L('blue', box(310, 160, 690, 800, 20), 'hole'), L('grey', disc(500, 870, 36))],
  '💻': () => [L('grey', box(150, 170, 850, 690, 40)), L('blue', box(210, 230, 790, 630, 16), 'hole'), L('greyLight', rounded(poly([60, 700], [940, 700], [990, 820], [10, 820]), 20)), L('grey', box(420, 740, 580, 780, 10), 'skip')],
  '📺': () => [L('grey', union(line([[500, 230], [350, 60]], 40), line([[500, 230], [650, 60]], 40))), L('inkSoft', box(80, 220, 920, 880, 70)), L('blue', box(150, 290, 720, 810, 40), 'hole'), L('greyLight', union(disc(820, 400, 50), disc(820, 560, 50)))],
  '✉': () => [...edged(box(80, 200, 920, 800, 50), 'white', 'greyLight', 36), L('greyLight', union(line([[110, 240], [500, 560], [890, 240]], 50)), 'ink')],
  '📌': () => [L('grey', line([[500, 560], [500, 940]], 40, true)), L('rose', union(box(330, 80, 670, 200, 50), poly([400, 200], [600, 200], [640, 450], [360, 450]), box(220, 440, 780, 560, 50)))],
  '🗑': () => [L('grey', union(box(160, 200, 840, 300, 30), box(400, 120, 600, 220, 30))), L('grey', poly([220, 330], [780, 330], [720, 930], [280, 930])), L('greyLight', union(line([[380, 420], [400, 840]], 40), line([[500, 420], [500, 840]], 40), line([[620, 420], [600, 840]], 40)), 'hole')],

  // Computer and media
  '💾': () => {
    const body = rounded(poly([100, 100], [800, 100], [900, 200], [900, 900], [100, 900]), 40);
    return [
      L('blue', body),
      S('blueDark', difference(body, move(body, -30, -30))),
      L('greyLight', box(270, 100, 680, 360, 20)),
      L('inkSoft', box(530, 150, 620, 320, 10)),
      L('white', box(210, 520, 790, 900, 30)),
      L('greyLight', union(line([[290, 640], [710, 640]], 40), line([[290, 760], [600, 760]], 40)), 'ink'),
    ];
  },
  '💿': () => {
    const d = disc(500, 500, 440);
    return [L('greyLight', d), L('cyanLight', pie(500, 500, 440, 440, 200, 250)), L('purple', pie(500, 500, 440, 440, 20, 60)), L('white', disc(500, 500, 150)), L('grey', difference(disc(500, 500, 110), disc(500, 500, 60)))].map((l) => ({ ...l, shape: difference(l.shape, disc(500, 500, 60)) }));
  },
  '📁': () => {
    const back = rounded(union(box(80, 220, 920, 860), box(80, 140, 420, 280)), 40);
    return [L('amberDark', back), L('amber', box(80, 340, 920, 860, 40))];
  },
  '📂': () => {
    const back = rounded(union(box(80, 220, 860, 860), box(80, 140, 420, 280)), 40);
    return [L('amberDark', back), ...edged(box(200, 240, 760, 700, 10), 'white', 'greyLight', 26), L('amber', rounded(poly([180, 440], [960, 440], [860, 860], [80, 860]), 40))];
  },
  '📄': () => [...page(), L('greyLight', union(line([[310, 420], [700, 420]], 44), line([[310, 560], [700, 560]], 44), line([[310, 700], [560, 700]], 44)), 'ink')],
  '📝': () => [
    ...page(),
    L('greyLight', union(line([[310, 420], [560, 420]], 44), line([[310, 560], [520, 560]], 44), line([[310, 700], [460, 700]], 44)), 'ink'),
    ...[
      L('amber' as Colour, turn(box(560, 300, 680, 760), 35, 620, 530)),
      L('rose' as Colour, turn(box(560, 220, 680, 320, 20), 35, 620, 530)),
      L('ink' as Colour, turn(poly([560, 760], [680, 760], [620, 880]), 35, 620, 530)),
    ],
  ],
  '📅': () => [
    ...edged(box(100, 150, 900, 920, 60), 'white'),
    L('rose', union(box(100, 150, 900, 360, 60), box(100, 250, 900, 360))),
    L('ink', union(...[0, 1, 2, 3].flatMap((c) => [0, 1, 2].map((r) => box(220 + c * 160, 460 + r * 140, 300 + c * 160, 530 + r * 140, 12))))),
    L('grey', union(box(260, 70, 330, 230, 30), box(670, 70, 740, 230, 30))),
  ],
  '⏰': () => [
    L('grey', union(line([[310, 780], [200, 920]], 60), line([[690, 780], [800, 920]], 60))),
    L('rose', union(turn(pie(230, 250, 170, 170, 180, 360), -35, 230, 250), turn(pie(770, 250, 170, 170, 180, 360), 35, 770, 250))),
    L('rose', disc(500, 540, 390)),
    ...clockFace(500, 540, 300),
  ],
  '⌚': () => [L('inkSoft', union(box(330, 40, 670, 960, 60))), L('grey', disc(500, 500, 300)), ...clockFace(500, 500, 230)],
  '📞': () => {
    const handset = union(arc(500, 560, 330, 330, 200, 340, 150, true), turn(box(90, 380, 330, 560, 60), -20, 210, 470), turn(box(670, 380, 910, 560, 60), 20, 790, 470));
    return [L('rose', turn(handset, -30, 500, 500))];
  },
  '💡': () => {
    const glass = union(disc(500, 400, 330), poly([330, 560], [670, 560], [620, 720], [380, 720]));
    return [L('amber', glass), L('amberLight', turn(disc(380, 300, 70, 110), 35, 380, 300)), L('grey', box(370, 720, 630, 900, 30)), L('inkSoft', union(line([[380, 780], [620, 780]], 30), line([[380, 840], [620, 840]], 30)))];
  },
  '🔑': () => {
    const ring = difference(disc(290, 500, 220), disc(290, 500, 90));
    const shaft = union(box(450, 450, 930, 560, 20), box(780, 540, 860, 690, 10), box(640, 540, 720, 650, 10));
    return [L('amber', turn(union(ring, shaft), -30, 500, 500))];
  },
  '🔒': () => padlock(false),
  '🔓': () => padlock(true),
  '🔔': () => {
    const bell = union(disc(500, 400, 300), poly([200, 400], [800, 400], [890, 790], [110, 790]));
    return [L('amber', rounded(bell, 40)), S('amberDark', difference(rounded(bell, 40), move(rounded(bell, 40), -40, -30))), L('brown', disc(500, 850, 100)), L('amber', box(80, 740, 920, 820, 40)), L('brown', disc(500, 110, 60))];
  },
  '🔊': () => [...speaker(), L('blue', union(arc(500, 500, 140, 140, -50, 50, 60), arc(500, 500, 270, 270, -50, 50, 60), arc(500, 500, 400, 400, -50, 50, 60)))],
  '🔇': () => [...speaker(), L('rose', union(line([[620, 360], [880, 640]], 80), line([[880, 360], [620, 640]], 80)))],
  '🎵': () => [L('blue', union(turn(disc(330, 760, 160, 120), -20, 330, 760), box(410, 140, 500, 760), poly([440, 140], [800, 260], [800, 420], [440, 300])))],
  '🎮': () => {
    const body = union(box(80, 300, 920, 640, 170), turn(box(110, 400, 340, 820, 110), 15, 225, 610), turn(box(660, 400, 890, 820, 110), -15, 775, 610));
    return [
      L('greyLight', body),
      S('grey', difference(body, move(body, -26, -26))),
      L('ink', union(box(200, 425, 400, 495, 12), box(265, 360, 335, 560, 12))),
      L('rose', disc(680, 400, 55)),
      L('blue', disc(780, 500, 55)),
    ];
  },
  '🕹': () => [L('inkSoft', box(140, 660, 860, 900, 60)), L('grey', box(140, 660, 860, 740, 40)), L('greyLight', box(460, 300, 540, 700)), L('rose', disc(500, 250, 160)), L('roseLight', disc(450, 200, 45)), L('rose', disc(720, 770, 50))],
  '🖥': () => [
    L('grey', union(box(60, 100, 940, 720, 50), box(420, 700, 580, 830), box(270, 820, 730, 900, 30))),
    L('blue', box(130, 170, 870, 650, 20), 'hole'),
    S('cyanLight', poly([170, 210], [400, 210], [170, 440])),
  ],
  '⌨': () => [
    L('grey', box(40, 270, 960, 760, 50)),
    L('white', union(...[0, 1, 2].flatMap((r) => Array.from({ length: 8 }, (_, c) => box(100 + c * 105 + (r % 2) * 40, 330 + r * 110, 180 + c * 105 + (r % 2) * 40, 410 + r * 110, 12)).slice(0, r % 2 ? 7 : 8)), box(250, 660, 750, 720, 12)), 'hole'),
  ],
  '🖱': () => {
    const body = disc(500, 560, 280, 380);
    return [L('grey', line([[500, 190], [500, 90], [650, 40]], 40)), ...edged(body, 'white', 'grey', 34), L('grey', union(line([[500, 190], [500, 470]], 30), line([[230, 470], [770, 470]], 30)))];
  },
  '🖨': () => [
    ...edged(box(250, 60, 750, 400, 10), 'white'),
    L('grey', box(60, 320, 940, 760, 60)),
    L('inkSoft', box(220, 620, 780, 690, 20)),
    ...edged(box(250, 660, 750, 950, 10), 'white'),
    L('green', disc(820, 420, 36)),
  ],
  '📷': () => [L('inkSoft', union(box(60, 260, 940, 860, 70), box(300, 170, 560, 300, 30))), L('grey', disc(520, 560, 250)), L('blue', disc(520, 560, 170), 'hole'), S('cyanLight', disc(470, 510, 50)), L('white', box(760, 330, 880, 400, 20))],

  // Fizz's own: drinks, trophy, saucer, thumbs up
  '☕': () => [
    L('greyLight', union(line([[380, 80], [340, 180], [380, 280]], 36), line([[560, 80], [520, 180], [560, 280]], 36))),
    L('white', union(arc(780, 590, 120, 120, -90, 90, 70), box(140, 360, 780, 860, 90))),
    S('greyLight', difference(box(140, 360, 780, 860, 90), move(box(140, 360, 780, 860, 90), -30, -30))),
    L('brown', box(190, 380, 730, 470, 30)),
  ],
  '🥤': () => [
    L('rose', line([[560, 220], [700, 40], [840, 40]], 50)),
    L('rose', poly([220, 300], [780, 300], [700, 950], [300, 950])),
    S('roseDark', poly([700, 300], [780, 300], [700, 950], [640, 950])),
    L('white', box(180, 230, 820, 330, 40)),
    L('white', poly([270, 540], [730, 540], [715, 660], [285, 660])),
  ],
  '🏆': () => [
    L('amber', union(arc(230, 330, 150, 170, 90, 270, 60), arc(770, 330, 150, 170, -90, 90, 60))),
    L('amber', union(pie(500, 150, 320, 470, 0, 180), box(180, 100, 820, 160))),
    S('amberDark', difference(pie(500, 150, 320, 470, 0, 180), pie(470, 120, 320, 470, 0, 180))),
    L('amber', box(440, 600, 560, 760)),
    L('brown', box(280, 760, 720, 940, 30)),
    L('amberLight', box(400, 820, 600, 880, 10)),
  ],
  '🛸': () => [
    L('cyan', disc(500, 400, 220, 200)),
    L('cyanLight', disc(430, 330, 50, 40)),
    L('grey', disc(500, 540, 440, 140)),
    L('greyLight', disc(500, 500, 400, 80)),
    L('amber', union(disc(260, 580, 40), disc(500, 610, 40), disc(740, 580, 40))),
  ],
  '👍': () => {
    const fist = box(330, 430, 850, 900, 100);
    const thumb = turn(box(330, 90, 520, 560, 95), -12, 430, 500);
    return [
      L('blue', box(100, 470, 330, 900, 30)),
      L('amber', union(fist, thumb)),
      L('amberDark', union(line([[560, 560], [800, 560]], 30), line([[560, 680], [800, 680]], 30), line([[560, 800], [800, 800]], 30))),
    ];
  },
};
