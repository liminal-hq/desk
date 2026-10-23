// The four Desk faces, in the order they are built and shown.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { FaceDef } from '../face';
import { fixed } from './fixed';
import { small } from './small';
import { title } from './title';
import { ui } from './ui';

export const FACES: FaceDef[] = [ui, title, fixed, small];
