// The characters every face must cover, and the code page 437 table that maps DOS bytes to them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const range = (from: number, to: number): string[] =>
  Array.from({ length: to - from + 1 }, (_, i) => String.fromCodePoint(from + i));

/** Printable ASCII, U+0020 to U+007E. */
export const ASCII = range(0x20, 0x7e);

/** Latin-1 Supplement, U+00A0 to U+00FF: every letter and symbol (U+00AD, the soft hyphen, included). */
export const LATIN1 = range(0xa0, 0xff);

/** Letters French needs beyond Latin-1. */
export const FRENCH_EXTRA = Array.from('ŒœŸ');

/** Box drawing: the single and double sets of code page 437, the mixed joins, and the rounded corners. */
export const BOX = Array.from(
  '─│┌┐└┘├┤┬┴┼═║╒╓╔╕╖╗╘╙╚╛╜╝╞╟╠╡╢╣╤╥╦╧╨╩╪╫╬╭╮╯╰',
);

/** Block elements and shades of code page 437. */
export const BLOCKS = Array.from('░▒▓█▀▄▌▐');

/** Typography and the symbols a shell draws, required in every face. */
export const SYMBOLS = Array.from('—–‘’‚“”„…•‹›€←↑→↓↔↕►◄▲▼●○■□✓✗');

/**
 * Windows code page 1252 (ANSI, what Windows 3.1 and 95 programs type in), bytes 0x80 to 0x9F, as
 * Unicode. The five undefined bytes are empty strings.
 */
export const CP1252_HIGH: string[] = [
  '€', '', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '', 'Ž', '',
  '', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '', 'ž', 'Ÿ',
];

/** Every printable character of code page 1252: ASCII, 0x80 to 0x9F where defined, and Latin-1. */
export const CP1252_PRINTABLE = [...ASCII, ...CP1252_HIGH.filter((c) => c), ...LATIN1];

/** Everything every face must have. */
export const REQUIRED = [...new Set([...CP1252_PRINTABLE, ...FRENCH_EXTRA, ...BOX, ...BLOCKS, ...SYMBOLS])];

/**
 * Code page 437 as Unicode, byte 0x00 to 0xFF. Bytes 0x01 to 0x1F and 0x7F are the pictures the
 * VGA ROM shows for them; byte 0x00 and 0xFF show as blanks (0xFF is a no-break space).
 */
export const CP437: string[] = Array.from(
  '\u0000☺☻♥♦♣♠•◘○◙♂♀♪♫☼►◄↕‼¶§▬↨↑↓→←∟↔▲▼' +
    ' !"#$%&\'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~⌂' +
    'ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»' +
    '░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀' +
    'αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■ ',
);

/** Every printable character of code page 437 (all but byte 0x00). */
export const CP437_PRINTABLE = CP437.slice(1);
