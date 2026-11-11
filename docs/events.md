# Event commands

What each event bytecode command is and how many operand words it takes. The body format
itself is in `formats.md`, "Event bytecode".

## Where the names come from

The executable registers every event command word with its Squirrel VM as a constant named
`ECT_*`, so that scripts can refer to them. Each registration is
`mov r8d, WORD; lea rdx, [name]; call add_constant`. `tools/ect_names.py` finds the
add-constant function from the `ECT_END` registration (`0x7f000000`) and lists every word
and name passed to it:

```sh
objdump -d -M intel --no-show-raw-insn GAME.exe > work/game.asm
tools/ect_names.py GAME.exe work/game.asm > work/ect.txt
tools/ebd_gaps.py "<install>/Event" work/ect.txt          # the table below
```

Escha & Logy DX registers 219 names. By group:

| Group | Top byte | Names | What |
|---|---|---|---|
| `p` | 0x70 | 6 | event start and the macro waits |
| `q` | 0x71 | 48 | field map: characters, gimmicks, fog, colour |
| `r` | 0x72 | 1 | battle |
| `s` | 0x73 | 103 | characters: position, motion, rotation, colour, effects, sound |
| `t` | 0x74 | 36 | camera and variable-index settings |
| `u` | 0x75 | 23 | event-wide: post effects, contrast, sound (`0x751.....`) |
| `x` | 0x78 | 1 | `ECT_SET_DEFCHARA_BCID` |
| | 0x7f | 1 | `ECT_END` |

The flags byte is part of the command's identity: `0x75100005` (`ECT_SOUND_BG_FADEOUT`) and
`0x75000005` are different commands. The executable compares whole words when it
dispatches them.

Of the 238 command words the events use, 132 are named; they account for 66% of all
command uses. The unnamed ones are not exposed to Squirrel. They are mostly in `r` (battle),
`u` and `w`, and their names have to come from their handlers.

`w1` (`0x77000001`) is the commonest word in the events (47,448 uses). It is always
followed by exactly one word, and it comes up in the middle of other commands' operands,
so it is probably an operand prefix (a variable or reference) rather than a command of its
own. A command that varies in the table below most likely has a `w1` operand in it.

## Operand counts

"Words after it" counts the words between a command and the next command word in the same
body, over every event in Escha & Logy DX. 232 of the 238 commands are followed by the same
number of words every time, which gives their operand count. The 6 that vary are listed
with each count seen.

| Word | Short | Name | Uses | Words after it (count) |
|---|---|---|---|---|
| `70000001` | `p1` | ECT_MACRO_TIME_WAIT | 8664 | 1 (8664) |
| `70000002` | `p2` | ECT_MACRO_FIELDMAP_CAMERA_MOVE_WAIT | 12 | 0 (12) |
| `70000003` | `p3` | ECT_MACRO_CHARA_WAIT | 3539 | 1 (3539) |
| `70000004` | `p4` | ECT_MACRO_CHARA_MARKER_WAIT | 23 | 1 (23) |
| `70000005` | `p5` | ECT_MACRO_CAMERA_WAIT | 313 | 0 (313) |
| `71000003` | `q3` | ECT_FIELDMAP_CHARA_ADDITION | 5895 | 2 (5895) |
| `71000004` | `q4` | ECT_FIELDMAP_CHARA_ADDITION_EX | 2398 | 14 (2398) |
| `71000005` | `q5` | ECT_FIELDMAP_CHARA_DELETE | 41 | 1 (41) |
| `71000006` | `q6` | ECT_FIELDMAP_CHARA_DELETE_ALL | 1109 | 0 (1109) |
| `71000007` | `q7` | ECT_FIELDMAP_CHARA_SET_EVENT_MODE | 5895 | 1 (5895) |
| `71000008` | `q8` | ECT_FIELDMAP_CHARA_COLLISION_FLAG | 8081 | 5 (8081) |
| `7100000a` | `q10` | ECT_FIELDMAP_EVENT_OBJECT_VISIBLE_SET | 771 | 1 (771) |
| `7100000b` | `q11` | ECT_FIELDMAP_NPC_ENEMY_RESET | 526 | 0 (526) |
| `7100000c` | `q12` | ECT_FIELDMAP_CHANGE_NOT_BGM | 621 | 1 (621) |
| `7100000d` | `q13` | ECT_FIELDMAP_GIMMICK_RESET | 17 | 0 (17) |
| `7100000f` | `q15` | ECT_FIELDMAP_MAPVISIBLE_SET | 53 | 3 (53) |
| `71000010` | `q16` | ECT_FIELDMAP_MAPANIMATION_EXEC | 2 | 6 (2) |
| `71000020` | `q32` | ECT_FIELDMAP_CHARA_TRANSPARENT_FOCUS | 249 | 5 (249) |
| `71000021` | `q33` | ECT_FIELDMAP_SET_FIELD_COLOR | 1 | 4 (1) |
| `71000024` | `q36` |  | 147 | 1 (147) |
| `7100002a` | `q42` | ECT_FIELDMAP_LIGHT_PARALLEL_SET | 15 | 6 (15) |
| `7100002b` | `q43` | ECT_FIELDMAP_LIGHT_PARALLEL_ORG | 18 | 1 (18) |
| `7100002c` | `q44` | ECT_FIELDMAP_LIGHT_AMBIENT_SET | 50 | 3 (50) |
| `7100002d` | `q45` | ECT_FIELDMAP_LIGHT_AMBIENT_ORG | 57 | 0 (57) |
| `71000032` | `q50` | ECT_FIELDMAP_CHARA_LIGHT_COLOR_SET | 34 | 4 (34) |
| `71000033` | `q51` | ECT_FIELDMAP_CHARA_LIGHT_COLOR_ORG | 38 | 0 (38) |
| `71000034` | `q52` | ECT_FIELDMAP_CHARA_LIM_LIGHT_COLOR_SET | 16 | 4 (16) |
| `71000035` | `q53` | ECT_FIELDMAP_CHARA_LIM_LIGHT_COLOR_ORG | 23 | 0 (23) |
| `71000036` | `q54` |  | 108 | 14 (108) |
| `71000037` | `q55` |  | 112 | 1 (112) |
| `71000038` | `q56` |  | 44 | 1 (44) |
| `71000039` | `q57` |  | 42 | 0 (42) |
| `72000004` | `r4` | ECT_BATTLE_ADD_EFFECT_ID | 1117 | 2 (1117) |
| `72000008` | `r8` |  | 1095 | 1 (1095) |
| `7200000d` | `r13` |  | 340 | 4 (340) |
| `7200000e` | `r14` |  | 681 | 6 (681) |
| `7200000f` | `r15` |  | 191 | 0 (191) |
| `72000010` | `r16` |  | 60 | 14 (60) |
| `72000011` | `r17` |  | 76 | 2 (76) |
| `72000012` | `r18` |  | 90 | 7 (90) |
| `72000013` | `r19` |  | 6 | 1 (6) |
| `72000014` | `r20` |  | 693 | 2 (693) |
| `72000016` | `r22` |  | 30 | 6 (30) |
| `72000018` | `r24` |  | 12 | 4 (12) |
| `7200001b` | `r27` |  | 3 | 1 (3) |
| `7200001c` | `r28` |  | 8 | 3 (8) |
| `7200001d` | `r29` |  | 3 | 1 (3) |
| `7200001e` | `r30` |  | 6 | 3 (6) |
| `7200001f` | `r31` |  | 1 | 4 (1) |
| `72000020` | `r32` |  | 13 | 6 (13) |
| `72000021` | `r33` |  | 1 | 1 (1) |
| `72000022` | `r34` |  | 2 | 3 (2) |
| `72000023` | `r35` |  | 41 | 2 (41) |
| `73000001` | `s1` | ECT_CHARA_SET_MARKER | 998 | 5 (998) |
| `73000002` | `s2` | ECT_CHARA_CAMERA_QUAKE_SET | 665 | 3 (665) |
| `73000003` | `s3` | ECT_CHARA_TIME_WAIT | 3849 | 2 (3849) |
| `73000004` | `s4` | ECT_CHARA_POS_SET | 20753 | 7 (20753) |
| `73000005` | `s5` | ECT_CHARA_MOTION_SET | 4435 | 4 (4435) |
| `73000006` | `s6` | ECT_CHARA_MOTION_WAIT | 2966 | 1 (2966) |
| `73000007` | `s7` | ECT_CHARA_MOTION_SET_DIRECT | 24 | 2 (24) |
| `73000008` | `s8` | ECT_CHARA_MOTION_SET_DIV | 858 | 4 (858) |
| `73000009` | `s9` | ECT_CHARA_MOTION_SET_SPEED | 1504 | 2 (1504) |
| `7300000a` | `s10` | ECT_CHARA_MOTION_SET_LOOP | 1604 | 4 (1604) |
| `7300000b` | `s11` | ECT_CHARA_MOTION_SET_AUTO_LOOP | 133 | 5 (133) |
| `7300000c` | `s12` | ECT_CHARA_MOTION_SPEED_MOVE_SET | 31 | 4 (31) |
| `7300000d` | `s13` | ECT_CHARA_MOTION_SET_OFFSET_ONLY | 68 | 3 (68) |
| `7300000e` | `s14` | ECT_CHARA_MOTION_SET_OFFSET_AUTO_LOOP | 44 | 3 (44) |
| `7300000f` | `s15` | ECT_CHARA_COLOR_SET | 908 | 5 (908) |
| `73000010` | `s16` | ECT_CHARA_COLOR_MOVE_SET | 533 | 7 (533) |
| `73000011` | `s17` | ECT_CHARA_COLOR_MOVE_WAIT | 83 | 1 (83) |
| `73000012` | `s18` | ECT_CHARA_MOVE_SET | 1717 | 13 (950), 11 (704), 19 (63) |
| `73000013` | `s19` | ECT_CHARA_MOVE_WAIT | 717 | 1 (717) |
| `73000014` | `s20` | ECT_CHARA_MOVE_SET_WAIT | 48 | 13 (48) |
| `73000015` | `s21` | ECT_CHARA_MOVE_PRE_WAIT | 741 | 2 (741) |
| `73000017` | `s23` | ECT_CHARA_ROT_SET | 4668 | 4 (4668) |
| `73000018` | `s24` | ECT_CHARA_ROT_SET_TARGET | 6809 | 7 (6809) |
| `73000019` | `s25` | ECT_CHARA_ROT_SET_TARGET_ALL | 100 | 7 (100) |
| `7300001a` | `s26` | ECT_CHARA_ROT_MOVE_SET | 1305 | 6 (1305) |
| `7300001b` | `s27` | ECT_CHARA_ROT_MOVE_SET_TARGET | 21 | 9 (21) |
| `7300001c` | `s28` | ECT_CHARA_ROT_MOVE_SET_TARGET_ALL | 8 | 9 (8) |
| `7300001d` | `s29` | ECT_CHARA_ROT_MOVE_WAIT | 174 | 1 (174) |
| `73000021` | `s33` | ECT_CHARA_ROT_STOP | 25 | 1 (25) |
| `73000022` | `s34` | ECT_CHARA_OFFSET_ROT_MOVE | 39 | 6 (39) |
| `73000023` | `s35` | ECT_CHARA_SCALE_SET | 762 | 4 (762) |
| `73000024` | `s36` | ECT_CHARA_SCALE_MOVE_SET | 246 | 6 (246) |
| `73000025` | `s37` | ECT_CHARA_SCALE_MOVE_WAIT | 55 | 1 (55) |
| `73000026` | `s38` | ECT_CHARA_EFF_ADD | 12013 | 25 (12013) |
| `73000027` | `s39` | ECT_CHARA_EFF_WAIT | 15 | 2 (15) |
| `73000029` | `s41` | ECT_CHARA_EFF_MOVE_SET | 245 | 14 (132), 12 (100), 20 (13) |
| `7300002a` | `s42` | ECT_CHARA_EFF_MOVE_WAIT | 158 | 2 (158) |
| `7300002c` | `s44` | ECT_CHARA_EFF_PRE_WAIT | 130 | 3 (130) |
| `7300002d` | `s45` | ECT_CHARA_EFF_STOP | 228 | 2 (228) |
| `7300002e` | `s46` | ECT_CHARA_EFF_KILL | 1406 | 2 (1406) |
| `7300002f` | `s47` |  | 19 | 17 (19) |
| `73000030` | `s48` | ECT_CHARA_NODEEFF_STOP | 2018 | 2 (2018) |
| `73000031` | `s49` | ECT_CHARA_NODEEFF_KILL | 1980 | 2 (1980) |
| `73000033` | `s51` | ECT_CHARA_LOOK_SET | 4 | 4 (4) |
| `73000035` | `s53` | ECT_CHARA_LOOK_MOVE_SET | 2 | 6 (2) |
| `73000039` | `s57` | ECT_CHARA_LOOK_CLEAR | 41 | 1 (41) |
| `7300003e` | `s62` | ECT_CHARA_SE_PLAY | 18 | 6 (18) |
| `73000040` | `s64` | ECT_CHARA_CHANGE_PARTS | 49 | 3 (49) |
| `73000042` | `s66` | ECT_CHARA_CHANGE_ADD_OBJ | 1 | 4 (1) |
| `73000044` | `s68` | ECT_CHARA_NODECHARA_ADD | 269 | 12 (269) |
| `73000045` | `s69` | ECT_CHARA_NODECHARA_DETACHCHILD | 8 | 2 (8) |
| `73000046` | `s70` | ECT_CHARA_NODECHARA_DETACH | 259 | 1 (259) |
| `73000047` | `s71` | ECT_CHARA_SET_DISP | 31 | 2 (31) |
| `73000050` | `s80` | ECT_CHARA_SET_FLAG | 6 | 3 (6) |
| `73000051` | `s81` | ECT_CHARA_SET_PROGRAM_MOTION_WAIT | 41 | 1 (41) |
| `73000053` | `s83` | ECT_CHARA_MULTI_MOTION_LOOP_CLEAR | 1 | 1 (1) |
| `73000054` | `s84` |  | 405 | 7 (405) |
| `73000055` | `s85` | ECT_CHARA_ROT_SET_COPY_TARGET_ALL | 11 | 7 (11) |
| `73000056` | `s86` | ECT_CHARA_ROT_MOVE_SET_COPY_TARGET | 2 | 9 (2) |
| `73000058` | `s88` | ECT_CHARA_FACIAL_MOTION_SET | 17 | 4 (17) |
| `7300005f` | `s95` | ECT_CHARA_LOAD_ONE_MOTION | 2 | 2 (2) |
| `73000062` | `s98` | ECT_CHARA_NODEEFF_ADD2 | 3896 | 19 (3896) |
| `73000063` | `s99` | ECT_CHARA_SE_NON_STREAM_PLAY | 4344 | 6 (4344) |
| `73000066` | `s102` | ECT_CHARA_VOICE_PLAY | 12 | 2 (12) |
| `73000070` | `s112` |  | 1 | 1 (1) |
| `73000071` | `s113` | ECT_CHARA_EFF_FADEOUT | 10 | 2 (10) |
| `73000072` | `s114` | ECT_CHARA_NODEEFF_FADEOUT | 115 | 2 (115) |
| `73000073` | `s115` | ECT_CHARA_OFFSET_ROT_MOVE_WITHOUT_SORTROT | 77 | 6 (77) |
| `73000074` | `s116` |  | 1604 | 4 (1604) |
| `73000075` | `s117` |  | 1632 | 1 (1632) |
| `73000076` | `s118` |  | 1 | 5 (1) |
| `73000077` | `s119` |  | 8 | 7 (8) |
| `73000079` | `s121` |  | 11 | 3 (11) |
| `7300007a` | `s122` |  | 3 | 14 (3) |
| `7300007b` | `s123` |  | 4 | 2 (4) |
| `74000001` | `t1` | ECT_CAMERA_C_TYPE_SET | 2785 | 1 (2781), 11 (4) |
| `74000002` | `t2` | ECT_CAMERA_TIME_WAIT | 127 | 1 (127) |
| `74000003` | `t3` | ECT_CAMERA_EYE_POS_SET | 931 | 6 (931) |
| `74000004` | `t4` | ECT_CAMERA_EYE_MOVE_SET | 2006 | 9 (1877), 17 (129) |
| `74000005` | `t5` | ECT_CAMERA_EYE_MOVE_WAIT | 1291 | 0 (1291) |
| `74000006` | `t6` | ECT_CAMERA_AT_POS_SET | 1004 | 6 (1004) |
| `74000007` | `t7` | ECT_CAMERA_AT_MOVE_SET | 1754 | 9 (1749), 17 (5) |
| `74000008` | `t8` | ECT_CAMERA_AT_MOVE_WAIT | 192 | 0 (192) |
| `74000009` | `t9` | ECT_CAMERA_EYEAT_POS_SET | 1613 | 12 (1613) |
| `7400000a` | `t10` | ECT_CAMERA_ROT_MOVE_SET | 895 | 4 (895) |
| `7400000b` | `t11` | ECT_CAMERA_ROT_MOVE_WAIT | 18 | 0 (18) |
| `7400000c` | `t12` | ECT_CAMERA_LENGTH_MOVE_SET | 41 | 3 (41) |
| `7400000d` | `t13` | ECT_CAMERA_LENGTH_MOVE_WAIT | 5 | 0 (5) |
| `7400000e` | `t14` | ECT_CAMERA_QUAKE_SET | 268 | 2 (268) |
| `74000010` | `t16` | ECT_CAMERA_QUAKE_STOP | 226 | 0 (226) |
| `74000018` | `t24` | ECT_CAMERA_FOV_SET | 18 | 1 (18) |
| `7400001a` | `t26` | ECT_CAMERA_FOV_MOVE | 12 | 3 (12) |
| `7400001d` | `t29` | ECT_CAMERA_MOVE_KILL | 1389 | 0 (1389) |
| `74100002` | `t2/0x10` |  | 11 | 0 (11) |
| `74100003` | `t3/0x10` |  | 431 | 2 (431) |
| `74100005` | `t5/0x10` |  | 7 | 2 (7) |
| `74100007` | `t7/0x10` |  | 428 | 2 (428) |
| `74100009` | `t9/0x10` |  | 428 | 2 (428) |
| `7410000b` | `t11/0x10` |  | 428 | 2 (428) |
| `7410000d` | `t13/0x10` |  | 431 | 4 (431) |
| `74100012` | `t18/0x10` | ECT_FIELDMAP_CAMERA_SET_FOLLOW_CHARA | 1144 | 2 (1144) |
| `74100013` | `t19/0x10` | ECT_FIELDMAP_CAMERA_DATA_EFFECT_SET | 1142 | 1 (1142) |
| `74100014` | `t20/0x10` |  | 4484 | 11 (4484) |
| `74100015` | `t21/0x10` |  | 17 | 12 (17) |
| `7420000c` | `t12/0x20` |  | 1 | 0 (1) |
| `74200011` | `t17/0x20` |  | 2 | 1 (2) |
| `74200012` | `t18/0x20` |  | 3 | 1 (3) |
| `74200013` | `t19/0x20` |  | 5 | 0 (5) |
| `74300001` | `t1/0x30` | ECT_VARIINDEX_INDEX_START | 195 | 0 (195) |
| `74300002` | `t2/0x30` | ECT_VARIINDEX_INDEX_SETTING | 6736 | 2 (6736) |
| `75000001` | `u1` |  | 596 | 0 (596) |
| `75000002` | `u2` |  | 741 | 1 (741) |
| `75000003` | `u3` |  | 1297 | 1 (1297) |
| `75000005` | `u5` |  | 806 | 1 (806) |
| `75000007` | `u7` |  | 199 | 1 (199) |
| `75000008` | `u8` |  | 99 | 3 (99) |
| `7500000b` | `u11` | ECT_EVENT_SET_TIME_EFFECT | 339 | 6 (339) |
| `7500000c` | `u12` | ECT_EVENT_WAIT_TIME_EFFECT | 66 | 1 (66) |
| `7500000d` | `u13` | ECT_EVENT_SET_POST_EFFECT | 2180 | 8 (1319), 4 (861) |
| `7500000e` | `u14` | ECT_EVENT_WAIT_POST_EFFECT | 637 | 0 (637) |
| `7500000f` | `u15` |  | 81 | 0 (81) |
| `75000010` | `u16` |  | 98 | 2 (98) |
| `75000011` | `u17` |  | 2 | 0 (2) |
| `75000012` | `u18` |  | 4990 | 2 (4990) |
| `75000015` | `u21` | ECT_EVENT_CONTRAST_WAIT | 138 | 1 (138) |
| `75000016` | `u22` | ECT_EVENT_CONTRAST_SET_MIX | 3159 | 3 (3159) |
| `75000019` | `u25` |  | 2 | 3 (2) |
| `7500001a` | `u26` |  | 93 | 4 (93) |
| `7500001b` | `u27` |  | 2 | 5 (2) |
| `7500001d` | `u29` |  | 24 | 5 (24) |
| `7500001e` | `u30` |  | 2 | 1 (2) |
| `7500001f` | `u31` | ECT_EVENT_VOICE_PLAY | 400 | 1 (400) |
| `75000022` | `u34` |  | 52 | 2 (52) |
| `75000023` | `u35` |  | 79 | 2 (79) |
| `75000024` | `u36` |  | 76 | 2 (76) |
| `75000026` | `u38` |  | 50 | 1 (50) |
| `7500002e` | `u46` |  | 18 | 3 (18) |
| `7500002f` | `u47` |  | 18 | 0 (18) |
| `75000030` | `u48` |  | 18 | 0 (18) |
| `75000033` | `u51` |  | 2 | 1 (2) |
| `75000034` | `u52` |  | 7 | 0 (7) |
| `75000035` | `u53` |  | 2 | 0 (2) |
| `75000037` | `u55` |  | 63 | 1 (63) |
| `75000038` | `u56` |  | 126 | 3 (126) |
| `75000043` | `u67` |  | 294 | 1 (294) |
| `75000044` | `u68` |  | 194 | 0 (194) |
| `7500004f` | `u79` |  | 3 | 0 (3) |
| `75000061` | `u97` |  | 65 | 3 (65) |
| `75000067` | `u103` | ECT_EVENT_POST_EFFECT_BLUR_SET_TARGET | 862 | 6 (862) |
| `75000069` | `u105` |  | 2 | 0 (2) |
| `7500006a` | `u106` |  | 2 | 0 (2) |
| `7500006b` | `u107` |  | 9 | 1 (9) |
| `7500006c` | `u108` |  | 9 | 0 (9) |
| `75000082` | `u130` |  | 2 | 2 (2) |
| `75000086` | `u134` |  | 22 | 1 (22) |
| `75000087` | `u135` |  | 5 | 0 (5) |
| `75000088` | `u136` |  | 5 | 0 (5) |
| `7500008b` | `u139` | ECT_EVENT_DEL_POST_EFFECT_ALL | 148 | 0 (148) |
| `75000091` | `u145` |  | 4 | 7 (4) |
| `75000092` | `u146` | ECT_EVENT_VOICE_PLAY_TRACK | 2 | 2 (2) |
| `75100001` | `u1/0x10` | ECT_SOUND_SE_PLAY | 5 | 5 (5) |
| `75100003` | `u3/0x10` | ECT_SOUND_BG_PLAY | 805 | 4 (805) |
| `75100004` | `u4/0x10` | ECT_SOUND_BG_STOP | 19 | 1 (19) |
| `75100005` | `u5/0x10` | ECT_SOUND_BG_FADEOUT | 1220 | 2 (1220) |
| `75100008` | `u8/0x10` |  | 234 | 5 (234) |
| `75200001` | `u1/0x20` | ECT_CONTRAST_SET | 515 | 3 (515) |
| `75200003` | `u3/0x20` | ECT_CONTRAST_WAIT | 141 | 1 (141) |
| `75200004` | `u4/0x20` | ECT_CONTRAST_SET_MIX | 8 | 3 (8) |
| `75300001` | `u1/0x30` |  | 243 | 1 (243) |
| `75300002` | `u2/0x30` |  | 84 | 2 (84) |
| `75300004` | `u4/0x30` |  | 3 | 1 (3) |
| `75300007` | `u7/0x30` |  | 14 | 7 (14) |
| `75350001` | `u1/0x35` |  | 17 | 1 (17) |
| `75400001` | `u1/0x40` |  | 29 | 4 (29) |
| `75400002` | `u2/0x40` |  | 29 | 3 (29) |
| `75400003` | `u3/0x40` |  | 88 | 2 (88) |
| `75400004` | `u4/0x40` |  | 3 | 2 (3) |
| `75400005` | `u5/0x40` |  | 7 | 2 (7) |
| `75400006` | `u6/0x40` |  | 3 | 0 (3) |
| `75400007` | `u7/0x40` |  | 3 | 0 (3) |
| `75400008` | `u8/0x40` |  | 134 | 0 (134) |
| `75400009` | `u9/0x40` |  | 3628 | 0 (3628) |
| `7540000a` | `u10/0x40` |  | 3628 | 0 (3628) |
| `77000001` | `w1` |  | 47448 | 1 (47448) |
| `77000004` | `w4` |  | 206 | 1 (206) |
