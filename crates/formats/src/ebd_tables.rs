//! Operand counts of Atelier Escha & Logy DX's event commands, measured over all of its
//! events (see docs/events.md). `None` marks the commands seen with more than one count;
//! the decoder runs those to the next command word.

pub static ESCHA: &[(u32, Option<u8>)] = &[
    (0x70000001, Some(1)), // ECT_MACRO_TIME_WAIT
    (0x70000002, Some(0)), // ECT_MACRO_FIELDMAP_CAMERA_MOVE_WAIT
    (0x70000003, Some(1)), // ECT_MACRO_CHARA_WAIT
    (0x70000004, Some(1)), // ECT_MACRO_CHARA_MARKER_WAIT
    (0x70000005, Some(0)), // ECT_MACRO_CAMERA_WAIT
    (0x71000003, Some(2)), // ECT_FIELDMAP_CHARA_ADDITION
    (0x71000004, Some(14)), // ECT_FIELDMAP_CHARA_ADDITION_EX
    (0x71000005, Some(1)), // ECT_FIELDMAP_CHARA_DELETE
    (0x71000006, Some(0)), // ECT_FIELDMAP_CHARA_DELETE_ALL
    (0x71000007, Some(1)), // ECT_FIELDMAP_CHARA_SET_EVENT_MODE
    (0x71000008, Some(5)), // ECT_FIELDMAP_CHARA_COLLISION_FLAG
    (0x7100000a, Some(1)), // ECT_FIELDMAP_EVENT_OBJECT_VISIBLE_SET
    (0x7100000b, Some(0)), // ECT_FIELDMAP_NPC_ENEMY_RESET
    (0x7100000c, Some(1)), // ECT_FIELDMAP_CHANGE_NOT_BGM
    (0x7100000d, Some(0)), // ECT_FIELDMAP_GIMMICK_RESET
    (0x7100000f, Some(3)), // ECT_FIELDMAP_MAPVISIBLE_SET
    (0x71000010, Some(6)), // ECT_FIELDMAP_MAPANIMATION_EXEC
    (0x71000020, Some(5)), // ECT_FIELDMAP_CHARA_TRANSPARENT_FOCUS
    (0x71000021, Some(4)), // ECT_FIELDMAP_SET_FIELD_COLOR
    (0x71000024, Some(1)),
    (0x7100002a, Some(6)), // ECT_FIELDMAP_LIGHT_PARALLEL_SET
    (0x7100002b, Some(1)), // ECT_FIELDMAP_LIGHT_PARALLEL_ORG
    (0x7100002c, Some(3)), // ECT_FIELDMAP_LIGHT_AMBIENT_SET
    (0x7100002d, Some(0)), // ECT_FIELDMAP_LIGHT_AMBIENT_ORG
    (0x71000032, Some(4)), // ECT_FIELDMAP_CHARA_LIGHT_COLOR_SET
    (0x71000033, Some(0)), // ECT_FIELDMAP_CHARA_LIGHT_COLOR_ORG
    (0x71000034, Some(4)), // ECT_FIELDMAP_CHARA_LIM_LIGHT_COLOR_SET
    (0x71000035, Some(0)), // ECT_FIELDMAP_CHARA_LIM_LIGHT_COLOR_ORG
    (0x71000036, Some(14)),
    (0x71000037, Some(1)),
    (0x71000038, Some(1)),
    (0x71000039, Some(0)),
    (0x72000004, Some(2)), // ECT_BATTLE_ADD_EFFECT_ID
    (0x72000008, Some(1)),
    (0x7200000d, Some(4)),
    (0x7200000e, Some(6)),
    (0x7200000f, Some(0)),
    (0x72000010, Some(14)),
    (0x72000011, Some(2)),
    (0x72000012, Some(7)),
    (0x72000013, Some(1)),
    (0x72000014, Some(2)),
    (0x72000016, Some(6)),
    (0x72000018, Some(4)),
    (0x7200001b, Some(1)),
    (0x7200001c, Some(3)),
    (0x7200001d, Some(1)),
    (0x7200001e, Some(3)),
    (0x7200001f, Some(4)),
    (0x72000020, Some(6)),
    (0x72000021, Some(1)),
    (0x72000022, Some(3)),
    (0x72000023, Some(2)),
    (0x73000001, Some(5)), // ECT_CHARA_SET_MARKER
    (0x73000002, Some(3)), // ECT_CHARA_CAMERA_QUAKE_SET
    (0x73000003, Some(2)), // ECT_CHARA_TIME_WAIT
    (0x73000004, Some(7)), // ECT_CHARA_POS_SET
    (0x73000005, Some(4)), // ECT_CHARA_MOTION_SET
    (0x73000006, Some(1)), // ECT_CHARA_MOTION_WAIT
    (0x73000007, Some(2)), // ECT_CHARA_MOTION_SET_DIRECT
    (0x73000008, Some(4)), // ECT_CHARA_MOTION_SET_DIV
    (0x73000009, Some(2)), // ECT_CHARA_MOTION_SET_SPEED
    (0x7300000a, Some(4)), // ECT_CHARA_MOTION_SET_LOOP
    (0x7300000b, Some(5)), // ECT_CHARA_MOTION_SET_AUTO_LOOP
    (0x7300000c, Some(4)), // ECT_CHARA_MOTION_SPEED_MOVE_SET
    (0x7300000d, Some(3)), // ECT_CHARA_MOTION_SET_OFFSET_ONLY
    (0x7300000e, Some(3)), // ECT_CHARA_MOTION_SET_OFFSET_AUTO_LOOP
    (0x7300000f, Some(5)), // ECT_CHARA_COLOR_SET
    (0x73000010, Some(7)), // ECT_CHARA_COLOR_MOVE_SET
    (0x73000011, Some(1)), // ECT_CHARA_COLOR_MOVE_WAIT
    (0x73000012, None), // ECT_CHARA_MOVE_SET
    (0x73000013, Some(1)), // ECT_CHARA_MOVE_WAIT
    (0x73000014, Some(13)), // ECT_CHARA_MOVE_SET_WAIT
    (0x73000015, Some(2)), // ECT_CHARA_MOVE_PRE_WAIT
    (0x73000017, Some(4)), // ECT_CHARA_ROT_SET
    (0x73000018, Some(7)), // ECT_CHARA_ROT_SET_TARGET
    (0x73000019, Some(7)), // ECT_CHARA_ROT_SET_TARGET_ALL
    (0x7300001a, Some(6)), // ECT_CHARA_ROT_MOVE_SET
    (0x7300001b, Some(9)), // ECT_CHARA_ROT_MOVE_SET_TARGET
    (0x7300001c, Some(9)), // ECT_CHARA_ROT_MOVE_SET_TARGET_ALL
    (0x7300001d, Some(1)), // ECT_CHARA_ROT_MOVE_WAIT
    (0x73000021, Some(1)), // ECT_CHARA_ROT_STOP
    (0x73000022, Some(6)), // ECT_CHARA_OFFSET_ROT_MOVE
    (0x73000023, Some(4)), // ECT_CHARA_SCALE_SET
    (0x73000024, Some(6)), // ECT_CHARA_SCALE_MOVE_SET
    (0x73000025, Some(1)), // ECT_CHARA_SCALE_MOVE_WAIT
    (0x73000026, Some(25)), // ECT_CHARA_EFF_ADD
    (0x73000027, Some(2)), // ECT_CHARA_EFF_WAIT
    (0x73000029, None), // ECT_CHARA_EFF_MOVE_SET
    (0x7300002a, Some(2)), // ECT_CHARA_EFF_MOVE_WAIT
    (0x7300002c, Some(3)), // ECT_CHARA_EFF_PRE_WAIT
    (0x7300002d, Some(2)), // ECT_CHARA_EFF_STOP
    (0x7300002e, Some(2)), // ECT_CHARA_EFF_KILL
    (0x7300002f, Some(17)),
    (0x73000030, Some(2)), // ECT_CHARA_NODEEFF_STOP
    (0x73000031, Some(2)), // ECT_CHARA_NODEEFF_KILL
    (0x73000033, Some(4)), // ECT_CHARA_LOOK_SET
    (0x73000035, Some(6)), // ECT_CHARA_LOOK_MOVE_SET
    (0x73000039, Some(1)), // ECT_CHARA_LOOK_CLEAR
    (0x7300003e, Some(6)), // ECT_CHARA_SE_PLAY
    (0x73000040, Some(3)), // ECT_CHARA_CHANGE_PARTS
    (0x73000042, Some(4)), // ECT_CHARA_CHANGE_ADD_OBJ
    (0x73000044, Some(12)), // ECT_CHARA_NODECHARA_ADD
    (0x73000045, Some(2)), // ECT_CHARA_NODECHARA_DETACHCHILD
    (0x73000046, Some(1)), // ECT_CHARA_NODECHARA_DETACH
    (0x73000047, Some(2)), // ECT_CHARA_SET_DISP
    (0x73000050, Some(3)), // ECT_CHARA_SET_FLAG
    (0x73000051, Some(1)), // ECT_CHARA_SET_PROGRAM_MOTION_WAIT
    (0x73000053, Some(1)), // ECT_CHARA_MULTI_MOTION_LOOP_CLEAR
    (0x73000054, Some(7)),
    (0x73000055, Some(7)), // ECT_CHARA_ROT_SET_COPY_TARGET_ALL
    (0x73000056, Some(9)), // ECT_CHARA_ROT_MOVE_SET_COPY_TARGET
    (0x73000058, Some(4)), // ECT_CHARA_FACIAL_MOTION_SET
    (0x7300005f, Some(2)), // ECT_CHARA_LOAD_ONE_MOTION
    (0x73000062, Some(19)), // ECT_CHARA_NODEEFF_ADD2
    (0x73000063, Some(6)), // ECT_CHARA_SE_NON_STREAM_PLAY
    (0x73000066, Some(2)), // ECT_CHARA_VOICE_PLAY
    (0x73000070, Some(1)),
    (0x73000071, Some(2)), // ECT_CHARA_EFF_FADEOUT
    (0x73000072, Some(2)), // ECT_CHARA_NODEEFF_FADEOUT
    (0x73000073, Some(6)), // ECT_CHARA_OFFSET_ROT_MOVE_WITHOUT_SORTROT
    (0x73000074, Some(4)),
    (0x73000075, Some(1)),
    (0x73000076, Some(5)),
    (0x73000077, Some(7)),
    (0x73000079, Some(3)),
    (0x7300007a, Some(14)),
    (0x7300007b, Some(2)),
    (0x74000001, None), // ECT_CAMERA_C_TYPE_SET
    (0x74000002, Some(1)), // ECT_CAMERA_TIME_WAIT
    (0x74000003, Some(6)), // ECT_CAMERA_EYE_POS_SET
    (0x74000004, None), // ECT_CAMERA_EYE_MOVE_SET
    (0x74000005, Some(0)), // ECT_CAMERA_EYE_MOVE_WAIT
    (0x74000006, Some(6)), // ECT_CAMERA_AT_POS_SET
    (0x74000007, None), // ECT_CAMERA_AT_MOVE_SET
    (0x74000008, Some(0)), // ECT_CAMERA_AT_MOVE_WAIT
    (0x74000009, Some(12)), // ECT_CAMERA_EYEAT_POS_SET
    (0x7400000a, Some(4)), // ECT_CAMERA_ROT_MOVE_SET
    (0x7400000b, Some(0)), // ECT_CAMERA_ROT_MOVE_WAIT
    (0x7400000c, Some(3)), // ECT_CAMERA_LENGTH_MOVE_SET
    (0x7400000d, Some(0)), // ECT_CAMERA_LENGTH_MOVE_WAIT
    (0x7400000e, Some(2)), // ECT_CAMERA_QUAKE_SET
    (0x74000010, Some(0)), // ECT_CAMERA_QUAKE_STOP
    (0x74000018, Some(1)), // ECT_CAMERA_FOV_SET
    (0x7400001a, Some(3)), // ECT_CAMERA_FOV_MOVE
    (0x7400001d, Some(0)), // ECT_CAMERA_MOVE_KILL
    (0x74100002, Some(0)),
    (0x74100003, Some(2)),
    (0x74100005, Some(2)),
    (0x74100007, Some(2)),
    (0x74100009, Some(2)),
    (0x7410000b, Some(2)),
    (0x7410000d, Some(4)),
    (0x74100012, Some(2)), // ECT_FIELDMAP_CAMERA_SET_FOLLOW_CHARA
    (0x74100013, Some(1)), // ECT_FIELDMAP_CAMERA_DATA_EFFECT_SET
    (0x74100014, Some(11)),
    (0x74100015, Some(12)),
    (0x7420000c, Some(0)),
    (0x74200011, Some(1)),
    (0x74200012, Some(1)),
    (0x74200013, Some(0)),
    (0x74300001, Some(0)), // ECT_VARIINDEX_INDEX_START
    (0x74300002, Some(2)), // ECT_VARIINDEX_INDEX_SETTING
    (0x75000001, Some(0)),
    (0x75000002, Some(1)),
    (0x75000003, Some(1)),
    (0x75000005, Some(1)),
    (0x75000007, Some(1)),
    (0x75000008, Some(3)),
    (0x7500000b, Some(6)), // ECT_EVENT_SET_TIME_EFFECT
    (0x7500000c, Some(1)), // ECT_EVENT_WAIT_TIME_EFFECT
    (0x7500000d, None), // ECT_EVENT_SET_POST_EFFECT
    (0x7500000e, Some(0)), // ECT_EVENT_WAIT_POST_EFFECT
    (0x7500000f, Some(0)),
    (0x75000010, Some(2)),
    (0x75000011, Some(0)),
    (0x75000012, Some(2)),
    (0x75000015, Some(1)), // ECT_EVENT_CONTRAST_WAIT
    (0x75000016, Some(3)), // ECT_EVENT_CONTRAST_SET_MIX
    (0x75000019, Some(3)),
    (0x7500001a, Some(4)),
    (0x7500001b, Some(5)),
    (0x7500001d, Some(5)),
    (0x7500001e, Some(1)),
    (0x7500001f, Some(1)), // ECT_EVENT_VOICE_PLAY
    (0x75000022, Some(2)),
    (0x75000023, Some(2)),
    (0x75000024, Some(2)),
    (0x75000026, Some(1)),
    (0x7500002e, Some(3)),
    (0x7500002f, Some(0)),
    (0x75000030, Some(0)),
    (0x75000033, Some(1)),
    (0x75000034, Some(0)),
    (0x75000035, Some(0)),
    (0x75000037, Some(1)),
    (0x75000038, Some(3)),
    (0x75000043, Some(1)),
    (0x75000044, Some(0)),
    (0x7500004f, Some(0)),
    (0x75000061, Some(3)),
    (0x75000067, Some(6)), // ECT_EVENT_POST_EFFECT_BLUR_SET_TARGET
    (0x75000069, Some(0)),
    (0x7500006a, Some(0)),
    (0x7500006b, Some(1)),
    (0x7500006c, Some(0)),
    (0x75000082, Some(2)),
    (0x75000086, Some(1)),
    (0x75000087, Some(0)),
    (0x75000088, Some(0)),
    (0x7500008b, Some(0)), // ECT_EVENT_DEL_POST_EFFECT_ALL
    (0x75000091, Some(7)),
    (0x75000092, Some(2)), // ECT_EVENT_VOICE_PLAY_TRACK
    (0x75100001, Some(5)), // ECT_SOUND_SE_PLAY
    (0x75100003, Some(4)), // ECT_SOUND_BG_PLAY
    (0x75100004, Some(1)), // ECT_SOUND_BG_STOP
    (0x75100005, Some(2)), // ECT_SOUND_BG_FADEOUT
    (0x75100008, Some(5)),
    (0x75200001, Some(3)), // ECT_CONTRAST_SET
    (0x75200003, Some(1)), // ECT_CONTRAST_WAIT
    (0x75200004, Some(3)), // ECT_CONTRAST_SET_MIX
    (0x75300001, Some(1)),
    (0x75300002, Some(2)),
    (0x75300004, Some(1)),
    (0x75300007, Some(7)),
    (0x75350001, Some(1)),
    (0x75400001, Some(4)),
    (0x75400002, Some(3)),
    (0x75400003, Some(2)),
    (0x75400004, Some(2)),
    (0x75400005, Some(2)),
    (0x75400006, Some(0)),
    (0x75400007, Some(0)),
    (0x75400008, Some(0)),
    (0x75400009, Some(0)),
    (0x7540000a, Some(0)),
    (0x77000001, Some(1)),
    (0x77000004, Some(1)),
    // Commands only the script macros use (Script/macro_*.nut), their counts as every use there
    // has them: FIELDMAP_CHARA_SET_BRAIN_MODE (11 uses), SET_DEFCHARA_BCID (3),
    // CHARA_SET_PROGRAM_ROT, CHARA_LOOK_SET_TARGET and CHARA_LOOK_MOVE_WAIT (1 each).
    (0x71000009, Some(2)),
    (0x78000004, Some(1)),
    (0x73000069, Some(1)),
    (0x73000034, Some(7)),
    (0x73000037, Some(1)),
];

/// Atelier Sophie 2's operand counts, measured from its 2,780 events (`barrel ebd-measure`: the
/// words to the next command, `None` where they vary) the way Escha's were; not yet checked
/// against its executable's handlers.
pub static SOPHIE2: &[(u32, Option<u8>)] = &[
    (0x70000001, Some(1)), // p1 ECT_MACRO_TIME_WAIT
    (0x70000002, Some(0)), // p2 ECT_MACRO_FIELDMAP_CAMERA_MOVE_WAIT
    (0x70000003, Some(1)), // p3 ECT_MACRO_CHARA_WAIT
    (0x70000005, Some(0)), // p5 ECT_MACRO_CAMERA_WAIT
    (0x71000002, Some(1)), // q2 ECT_FIELDMAP_CHANGE_NOT_BGM
    (0x71000004, Some(1)), // q4 ECT_FIELDMAP_CHARA_ADDITION
    (0x71000005, Some(13)), // q5 ECT_FIELDMAP_CHARA_ADDITION_EX
    (0x71000007, Some(1)), // q7 ECT_FIELDMAP_CHARA_DELETE
    (0x71000008, Some(0)), // q8 ECT_FIELDMAP_CHARA_DELETE_ALL
    (0x71000009, Some(1)), // q9 ECT_FIELDMAP_CHARA_SET_EVENT_MODE
    (0x7100000a, Some(5)), // q10 ECT_FIELDMAP_CHARA_COLLISION_FLAG
    (0x7100000b, Some(3)), // q11 ECT_FIELDMAP_GET_CUR_WEP_AND_INDEX_SETTING
    (0x7100000c, Some(0)), // q12 ECT_FIELDMAP_NPC_ENEMY_RESET
    (0x7100000d, Some(0)), // q13 ECT_FIELDMAP_NPC_ENEMY_RESET_SYNC
    (0x7100000f, Some(0)), // q15 ECT_FIELDMAP_GIMMICK_RESET
    (0x71000018, Some(2)), // q24 ECT_FIELDMAP_GIMMICK_SET_VISIBLE
    (0x71000019, Some(1)), // q25 ECT_FIELDMAP_GIMMICK_ALL_NOT_VISIBLE
    (0x7100001a, Some(1)), // q26 ECT_FIELDMAP_GIMMICK_ALL_VISIBLE_RECOVER
    (0x7100001b, Some(0)), // q27
    (0x7100001c, Some(0)), // q28
    (0x7100001e, Some(3)), // q30
    (0x7100001f, Some(14)), // q31
    (0x71000020, Some(2)), // q32
    (0x71000021, Some(2)), // q33 ECT_FIELDMAP_SET_DISP_SHADOW
    (0x71000024, Some(4)), // q36 ECT_FIELDMAP_SET_FIELD_COLOR
    (0x71000025, Some(6)), // q37 ECT_FIELDMAP_SET_FIELD_COLOR_MOVE
    (0x71000026, Some(0)), // q38 ECT_FIELDMAP_WAIT_FIELD_COLOR_MOVE
    (0x71000028, Some(2)), // q40
    (0x71000029, Some(0)), // q41
    (0x7100002a, Some(7)), // q42 ECT_FIELDMAP_LIGHT_PARALLEL_SET
    (0x7100002b, Some(1)), // q43 ECT_FIELDMAP_LIGHT_PARALLEL_ORG
    (0x7100002c, Some(3)), // q44 ECT_FIELDMAP_LIGHT_AMBIENT_SET
    (0x7100002d, Some(0)), // q45 ECT_FIELDMAP_LIGHT_AMBIENT_ORG
    (0x7100002e, Some(14)), // q46
    (0x71000030, Some(1)), // q48
    (0x71000031, Some(0)), // q49
    (0x7100003b, Some(1)), // q59
    (0x7100003c, Some(2)), // q60
    (0x7100003d, Some(0)), // q61
    (0x7100003e, Some(0)), // q62
    (0x7100003f, Some(0)), // q63
    (0x7100004f, Some(6)), // q79
    (0x72000004, Some(2)), // r4 ECT_BATTLE_ADD_EFFECT_ID
    (0x72000008, Some(1)), // r8
    (0x7200000a, Some(0)), // r10
    (0x7200000d, Some(4)), // r13
    (0x7200000e, Some(6)), // r14
    (0x7200000f, Some(0)), // r15
    (0x72000010, Some(3)), // r16
    (0x72000011, Some(2)), // r17
    (0x72000012, Some(7)), // r18
    (0x72000013, Some(1)), // r19
    (0x72000014, Some(2)), // r20
    (0x72000015, Some(1)), // r21
    (0x72000016, Some(6)), // r22
    (0x72000017, Some(1)), // r23
    (0x7200001c, Some(0)), // r28
    (0x7200001d, Some(1)), // r29
    (0x7200001f, Some(2)), // r31
    (0x72000020, Some(1)), // r32
    (0x72000024, Some(8)), // r36
    (0x72000028, None), // r40
    (0x72000029, Some(0)), // r41
    (0x7200002a, Some(1)), // r42
    (0x7200002b, Some(1)), // r43
    (0x73000001, Some(2)), // s1 ECT_CHARA_TIME_WAIT
    (0x73000002, Some(7)), // s2 ECT_CHARA_POS_SET
    (0x73000003, Some(7)), // s3
    (0x73000005, None), // s5 ECT_CHARA_MOVE_SET
    (0x73000006, Some(1)), // s6 ECT_CHARA_MOVE_WAIT
    (0x73000008, Some(2)), // s8 ECT_CHARA_MOVE_PRE_WAIT
    (0x73000009, None), // s9
    (0x7300000b, None), // s11
    (0x7300000d, Some(4)), // s13 ECT_CHARA_ROT_SET
    (0x7300000e, Some(7)), // s14 ECT_CHARA_ROT_SET_TARGET
    (0x7300000f, Some(7)), // s15 ECT_CHARA_ROT_SET_TARGET_ALL
    (0x73000010, Some(6)), // s16 ECT_CHARA_ROT_MOVE_SET
    (0x73000011, Some(9)), // s17 ECT_CHARA_ROT_MOVE_SET_TARGET
    (0x73000012, Some(9)), // s18 ECT_CHARA_ROT_MOVE_SET_TARGET_ALL
    (0x73000013, Some(1)), // s19 ECT_CHARA_ROT_MOVE_WAIT
    (0x73000014, Some(2)), // s20 ECT_CHARA_ROT_SET_DEFAULT
    (0x73000017, Some(1)), // s23 ECT_CHARA_ROT_STOP
    (0x73000018, Some(6)), // s24 ECT_CHARA_OFFSET_ROT_MOVE
    (0x73000019, Some(6)), // s25 ECT_CHARA_OFFSET_ROT_MOVE_WITHOUT_SORTROT
    (0x7300001a, Some(7)), // s26
    (0x7300001b, Some(7)), // s27 ECT_CHARA_ROT_SET_COPY_TARGET_ALL
    (0x7300001c, Some(9)), // s28 ECT_CHARA_ROT_MOVE_SET_COPY_TARGET
    (0x7300001e, Some(1)), // s30 ECT_CHARA_SET_PROGRAM_ROT
    (0x7300001f, Some(8)), // s31
    (0x73000020, Some(6)), // s32
    (0x73000022, None), // s34 ECT_CHARA_MOTION_SET
    (0x73000023, Some(5)), // s35 ECT_CHARA_MOTION_SET_WITHOUT_ACTION
    (0x73000024, Some(1)), // s36 ECT_CHARA_MOTION_WAIT
    (0x73000026, Some(4)), // s38 ECT_CHARA_MOTION_SET_DIV
    (0x73000027, Some(2)), // s39 ECT_CHARA_MOTION_SET_SPEED
    (0x73000028, Some(5)), // s40 ECT_CHARA_MOTION_SET_LOOP
    (0x73000029, Some(5)), // s41 ECT_CHARA_MOTION_SET_LOOP_WITHOUT_ACTION
    (0x7300002b, Some(4)), // s43 ECT_CHARA_MOTION_SPEED_MOVE_SET
    (0x7300002e, Some(7)), // s46
    (0x7300002f, Some(2)), // s47
    (0x73000032, Some(5)), // s50 ECT_CHARA_COLOR_SET
    (0x73000033, Some(7)), // s51 ECT_CHARA_COLOR_MOVE_SET
    (0x73000034, Some(1)), // s52 ECT_CHARA_COLOR_MOVE_WAIT
    (0x73000037, Some(2)), // s55 ECT_CHARA_SET_DISP
    (0x73000038, Some(3)), // s56 ECT_CHARA_SET_PARTS_MODEL_VISIBLE
    (0x7300003a, Some(13)), // s58
    (0x7300003b, Some(1)), // s59
    (0x7300003c, Some(7)), // s60
    (0x7300003d, Some(1)), // s61
    (0x7300003e, Some(4)), // s62 ECT_CHARA_SCALE_SET
    (0x7300003f, Some(6)), // s63 ECT_CHARA_SCALE_MOVE_SET
    (0x73000040, Some(1)), // s64 ECT_CHARA_SCALE_MOVE_WAIT
    (0x73000045, Some(25)), // s69 ECT_CHARA_EFF_ADD
    (0x73000046, Some(2)), // s70 ECT_CHARA_EFF_WAIT
    (0x73000048, None), // s72 ECT_CHARA_EFF_MOVE_SET
    (0x73000049, Some(2)), // s73 ECT_CHARA_EFF_MOVE_WAIT
    (0x7300004c, Some(2)), // s76 ECT_CHARA_EFF_STOP
    (0x7300004d, Some(2)), // s77 ECT_CHARA_EFF_KILL
    (0x7300004e, Some(2)), // s78 ECT_CHARA_EFF_FADEOUT
    (0x7300004f, Some(21)), // s79 ECT_CHARA_NODEEFF_ADD
    (0x73000050, Some(2)), // s80 ECT_CHARA_NODEEFF_STOP
    (0x73000051, Some(2)), // s81 ECT_CHARA_NODEEFF_KILL
    (0x73000052, Some(2)), // s82 ECT_CHARA_NODEEFF_FADEOUT
    (0x73000053, Some(2)), // s83
    (0x73000058, Some(4)), // s88 ECT_CHARA_LOOK_SET
    (0x73000059, Some(7)), // s89 ECT_CHARA_LOOK_SET_TARGET
    (0x7300005a, Some(6)), // s90 ECT_CHARA_LOOK_MOVE_SET
    (0x7300005b, Some(9)), // s91 ECT_CHARA_LOOK_MOVE_SET_TARGET
    (0x7300005c, Some(1)), // s92 ECT_CHARA_LOOK_MOVE_WAIT
    (0x7300005e, Some(1)), // s94 ECT_CHARA_LOOK_CLEAR
    (0x7300005f, Some(2)), // s95
    (0x73000063, Some(12)), // s99 ECT_CHARA_NODECHARA_ADD
    (0x73000064, Some(2)), // s100 ECT_CHARA_NODECHARA_DETACHCHILD
    (0x73000065, Some(1)), // s101 ECT_CHARA_NODECHARA_DETACH
    (0x73000066, Some(2)), // s102 ECT_CHARA_NODECHARA_DETACHCHILD_VISIBLE_OFF
    (0x73000069, Some(4)), // s105 ECT_CHARA_FACIAL_MOTION_SET
    (0x7300006a, Some(2)), // s106 ECT_CHARA_FACIAL_MOUTH_MOTION_SET_SPEED
    (0x7300006b, Some(3)), // s107 ECT_CHARA_FACIAL_BROW_MOTION_SET_LOOP
    (0x7300006c, Some(3)), // s108 ECT_CHARA_FACIAL_EYE_MOTION_SET_LOOP
    (0x7300006d, Some(3)), // s109 ECT_CHARA_FACIAL_MOUTH_MOTION_SET_LOOP
    (0x7300006e, Some(4)), // s110 ECT_CHARA_FACIAL_BROW_MOTION_SET_LOOP_WITH_BLEND
    (0x7300006f, Some(4)), // s111 ECT_CHARA_FACIAL_EYE_MOTION_SET_LOOP_WITH_BLEND
    (0x73000070, Some(3)), // s112 ECT_CHARA_FACIAL_MOUTH_SET_DIV
    (0x73000071, Some(2)), // s113
    (0x73000074, Some(3)), // s116
    (0x73000075, Some(4)), // s117 ECT_CHARA_FACIAL_EXP_MOUTH_MOTION_SET_LOOP_WITH_BLEND
    (0x73000076, Some(6)), // s118 ECT_CHARA_SE_NON_STREAM_PLAY
    (0x73000079, Some(3)), // s121 ECT_CHARA_VOICE_PLAY
    (0x7300007a, Some(7)), // s122 ECT_CHARA_VOICE_PLAY_RAND
    (0x7300007c, Some(6)), // s124 ECT_CHARA_SE_PLAY
    (0x7300007d, Some(3)), // s125 ECT_CHARA_SE_STOP
    (0x73000080, Some(1)), // s128 ECT_CHARA_CLOTH_RESET
    (0x73000081, Some(1)), // s129
    (0x73000082, Some(2)), // s130
    (0x73000084, Some(2)), // s132
    (0x73000089, Some(3)), // s137 ECT_CHARA_CLOTH_SET_WARMUP_TIME
    (0x73000093, Some(2)), // s147
    (0x73000094, Some(7)), // s148 ECT_CHARA_WEAPON_ACTION
    (0x73000095, Some(2)), // s149 ECT_CHARA_SET_WEAPON_MOTION_SPEED
    (0x7300009a, Some(2)), // s154 ECT_CHARA_VISIBLE_WEAPON_OPTION
    (0x7300009b, Some(2)), // s155 ECT_CHARA_VISIBLE_WEAPON
    (0x7300009c, Some(3)), // s156
    (0x7300009d, Some(3)), // s157
    (0x730000a2, Some(5)), // s162
    (0x730000a4, None), // s164
    (0x730000a6, Some(2)), // s166
    (0x730000a9, Some(7)), // s169
    (0x730000aa, Some(11)), // s170
    (0x730000ac, Some(8)), // s172
    (0x730000ad, Some(5)), // s173 ECT_CHARA_ACTION_SET
    (0x730000ae, Some(7)), // s174 ECT_CHARA_ACTION_SET_DIV
    (0x730000af, Some(11)), // s175 ECT_CHARA_SET_MARKER
    (0x730000b0, None), // s176 ECT_CHARA_CAMERA_QUAKE_SET
    (0x730000b1, None), // s177 ECT_CHARA_CAMERA_QUAKE_PARAM_SET
    (0x730000b6, None), // s182 ECT_CHARA_CAMERA_QUAKE_WIDTH_SET
    (0x730000b7, Some(4)), // s183 ECT_CHARA_CAMERA_QUAKE_INTERVAL_SET
    (0x730000b9, Some(10)), // s185 ECT_CHARA_CAMERA_QUAKE_TARGET_SET
    (0x730000ba, None), // s186 ECT_CHARA_CAMERA_QUAKE_TARGET_THRESHOLD_SET
    (0x730000bb, Some(2)), // s187 ECT_CHARA_CAMERA_QUAKE_ID_STOP
    (0x730000be, Some(2)), // s190 ECT_CHARA_SET_FLAG
    (0x730000c0, None), // s192
    (0x730000c1, Some(1)), // s193
    (0x730000c2, Some(2)), // s194
    (0x730000c3, Some(2)), // s195
    (0x730000c5, Some(1)), // s197
    (0x730000cb, Some(2)), // s203
    (0x730000cc, Some(2)), // s204
    (0x730000d5, Some(2)), // s213
    (0x730000d6, Some(1)), // s214
    (0x730000d7, Some(2)), // s215
    (0x730000d8, Some(1)), // s216
    (0x730000e2, Some(1)), // s226 ECT_CHARA_SYNCHRO_INDEX_WAIT
    (0x730000e7, Some(1)), // s231 ECT_CHARA_FACIAL_ALL_MOTION_RESET
    (0x730000ec, Some(13)), // s236
    (0x730000f1, Some(13)), // s241 ECT_CHARA_3DSE_PLAY
    (0x730000f2, Some(2)), // s242
    (0x730000f7, Some(1)), // s247
    (0x730000f8, Some(8)), // s248
    (0x730000fc, Some(1)), // s252
    (0x730000fd, Some(8)), // s253
    (0x73000101, None), // s257
    (0x73000102, Some(2)), // s258
    (0x73000103, Some(2)), // s259
    (0x73000104, Some(2)), // s260
    (0x73000106, Some(19)), // s262
    (0x74000001, Some(1)), // t1 ECT_CAMERA_C_TYPE_SET
    (0x74000002, Some(1)), // t2 ECT_CAMERA_TIME_WAIT
    (0x74000003, Some(6)), // t3 ECT_CAMERA_EYE_POS_SET
    (0x74000004, None), // t4 ECT_CAMERA_EYE_MOVE_SET
    (0x74000005, Some(0)), // t5 ECT_CAMERA_EYE_MOVE_WAIT
    (0x74000006, Some(1)), // t6 ECT_CAMERA_EYE_MOVE_PRE_WAIT
    (0x74000007, Some(6)), // t7 ECT_CAMERA_AT_POS_SET
    (0x74000008, None), // t8 ECT_CAMERA_AT_MOVE_SET
    (0x74000009, Some(0)), // t9 ECT_CAMERA_AT_MOVE_WAIT
    (0x7400000a, Some(1)), // t10 ECT_CAMERA_AT_MOVE_PRE_WAIT
    (0x7400000b, Some(0)), // t11 ECT_CAMERA_MOVE_KILL
    (0x7400000c, Some(12)), // t12 ECT_CAMERA_EYEAT_POS_SET
    (0x7400000d, Some(1)), // t13 ECT_CAMERA_SPRING_EFFECT_SET
    (0x7400000f, Some(0)), // t15 ECT_CAMERA_ROT_ALL_MOVE_WAIT
    (0x74000010, Some(10)), // t16
    (0x74000011, Some(9)), // t17
    (0x74000012, Some(4)), // t18 ECT_CAMERA_ROT_MOVE_SET
    (0x74000013, Some(0)), // t19 ECT_CAMERA_ROT_MOVE_WAIT
    (0x74000014, Some(3)), // t20 ECT_CAMERA_LENGTH_MOVE_SET
    (0x74000015, Some(0)), // t21 ECT_CAMERA_LENGTH_MOVE_WAIT
    (0x7400001a, Some(1)), // t26 ECT_CAMERA_FOV_SET
    (0x7400001b, Some(0)), // t27 ECT_CAMERA_FOV_RESET
    (0x7400001c, Some(3)), // t28 ECT_CAMERA_FOV_MOVE
    (0x7400001d, Some(0)), // t29 ECT_CAMERA_FOV_WAIT
    (0x7400001e, Some(0)), // t30 ECT_CAMERA_FOV_STOP
    (0x7400001f, Some(1)), // t31 ECT_CAMERA_CHANGE_FOLLOW
    (0x74000022, Some(2)), // t34 ECT_CAMERA_QUAKE_SET
    (0x74000023, Some(0)), // t35 ECT_CAMERA_QUAKE_WAIT
    (0x74000024, Some(7)), // t36 ECT_CAMERA_QUAKE_PARAM_SET
    (0x74000028, Some(5)), // t40 ECT_CAMERA_QUAKE_HANDSHAKE_SET
    (0x74000029, Some(4)), // t41 ECT_CAMERA_QUAKE_WIDTH_SET
    (0x7400002a, Some(3)), // t42 ECT_CAMERA_QUAKE_INTERVAL_SET
    (0x7400002e, Some(1)), // t46 ECT_CAMERA_QUAKE_ID_STOP
    (0x7400002f, Some(0)), // t47 ECT_CAMERA_QUAKE_STOP
    (0x74000030, Some(1)), // t48 ECT_CAMERA_CHANGE_MACRO_FOLLOW
    (0x74000031, Some(3)), // t49 ECT_CAMERA_EYE_MOVE_CIRCLE
    (0x74000033, Some(2)), // t51
    (0x74000034, Some(0)), // t52
    (0x74000035, Some(0)), // t53
    (0x74000039, Some(3)), // t57 ECT_CAMERA_BATTLE_RESET
    (0x7400003a, Some(9)), // t58 ECT_CAMERA_ROT_ALL_MOVE_DIRECTION_SET
    (0x74100002, Some(0)), // t2/0x10
    (0x74100004, Some(2)), // t4/0x10
    (0x74100016, Some(2)), // t22/0x10
    (0x74100017, Some(2)), // t23/0x10 ECT_FIELDMAP_CAMERA_SET_FOLLOW_CHARA
    (0x74100018, Some(1)), // t24/0x10 ECT_FIELDMAP_CAMERA_DATA_EFFECT_SET
    (0x74100019, Some(11)), // t25/0x10 ECT_FIELDMAP_CAMERA_ALL_ABSOLUTE
    (0x7420000c, Some(0)), // t12/0x20
    (0x74200010, Some(1)), // t16/0x20
    (0x74200011, Some(1)), // t17/0x20
    (0x74200012, Some(1)), // t18/0x20
    (0x74200013, Some(0)), // t19/0x20
    (0x74300001, Some(0)), // t1/0x30 ECT_VARIINDEX_INDEX_START
    (0x74300002, Some(2)), // t2/0x30 ECT_VARIINDEX_INDEX_SETTING
    (0x75000001, Some(0)), // u1
    (0x75000002, None), // u2
    (0x75000003, Some(0)), // u3
    (0x75000006, Some(1)), // u6
    (0x75000007, Some(1)), // u7
    (0x75000008, Some(1)), // u8
    (0x75000009, Some(0)), // u9
    (0x7500000a, Some(1)), // u10
    (0x7500000b, Some(0)), // u11
    (0x7500000c, Some(0)), // u12
    (0x7500000f, Some(2)), // u15
    (0x75000010, Some(0)), // u16
    (0x75000011, Some(2)), // u17
    (0x75000014, Some(2)), // u20
    (0x75000015, Some(1)), // u21
    (0x75000016, None), // u22
    (0x75000018, Some(0)), // u24
    (0x75000019, Some(4)), // u25
    (0x7500001c, Some(1)), // u28 ECT_EVENT_VOICE_PLAY
    (0x7500001f, Some(4)), // u31
    (0x75000020, Some(0)), // u32
    (0x75000024, Some(1)), // u36
    (0x75000025, Some(1)), // u37
    (0x75000028, None), // u40 ECT_EVENT_SET_POST_EFFECT_WITH_ID
    (0x7500002c, None), // u44 ECT_EVENT_SET_POST_EFFECT
    (0x7500002d, Some(0)), // u45 ECT_EVENT_WAIT_POST_EFFECT
    (0x7500002e, Some(0)), // u46 ECT_EVENT_DEL_POST_EFFECT_ALL
    (0x7500002f, None), // u47 ECT_EVENT_POST_EFFECT_SET_TARGET
    (0x75000031, Some(3)), // u49 ECT_EVENT_CONTRAST_SET_MIX
    (0x75000032, Some(1)), // u50 ECT_EVENT_CONTRAST_WAIT
    (0x75000034, None), // u52
    (0x75000035, Some(1)), // u53
    (0x75000036, Some(1)), // u54
    (0x75000041, Some(1)), // u65
    (0x75000042, Some(3)), // u66
    (0x7500004c, Some(10)), // u76
    (0x7500004d, Some(4)), // u77
    (0x7500004e, Some(3)), // u78
    (0x7500004f, Some(2)), // u79
    (0x75000050, Some(3)), // u80
    (0x75000051, Some(0)), // u81
    (0x75000052, Some(0)), // u82
    (0x75000057, Some(2)), // u87
    (0x75000058, Some(10)), // u88
    (0x75000059, Some(7)), // u89
    (0x7500005a, Some(0)), // u90
    (0x7500005b, Some(1)), // u91
    (0x7500005c, Some(1)), // u92
    (0x7500005d, Some(0)), // u93
    (0x7500005e, Some(1)), // u94
    (0x75000060, Some(1)), // u96
    (0x75000061, Some(2)), // u97
    (0x75000062, Some(3)), // u98
    (0x75000068, Some(1)), // u104
    (0x7500006e, Some(0)), // u110
    (0x7500006f, Some(5)), // u111
    (0x75000073, Some(1)), // u115
    (0x75000074, Some(1)), // u116
    (0x75000075, Some(0)), // u117
    (0x75000076, Some(0)), // u118
    (0x75000089, Some(0)), // u137
    (0x7500008a, Some(0)), // u138
    (0x7500008b, Some(0)), // u139
    (0x7500008e, Some(1)), // u142
    (0x7500008f, Some(1)), // u143
    (0x75000090, None), // u144
    (0x75000091, Some(1)), // u145
    (0x75000092, Some(6)), // u146 ECT_EVENT_SET_TIME_EFFECT
    (0x75000093, Some(1)), // u147
    (0x75000094, Some(1)), // u148 ECT_EVENT_WAIT_TIME_EFFECT
    (0x75000098, Some(0)), // u152
    (0x7500009a, Some(0)), // u154
    (0x7500009b, Some(1)), // u155
    (0x7500009d, Some(0)), // u157
    (0x7500009e, Some(0)), // u158
    (0x7500009f, Some(0)), // u159
    (0x750000a0, Some(0)), // u160
    (0x75100001, Some(5)), // u1/0x10 ECT_SOUND_SE_PLAY
    (0x75100002, Some(1)), // u2/0x10 ECT_SOUND_SE_STOP
    (0x75100003, Some(4)), // u3/0x10 ECT_SOUND_BG_PLAY
    (0x75100004, Some(1)), // u4/0x10 ECT_SOUND_BG_STOP
    (0x75100005, Some(2)), // u5/0x10 ECT_SOUND_BG_FADEOUT
    (0x75100006, Some(3)), // u6/0x10 ECT_SOUND_BG_FADE
    (0x75100009, Some(0)), // u9/0x10
    (0x7510000a, Some(0)), // u10/0x10 ECT_SOUND_ENV_SE_PLAY
    (0x7510000b, Some(0)), // u11/0x10 ECT_SOUND_ENV_SE_STOP
    (0x7510000c, Some(1)), // u12/0x10 ECT_SOUND_ENV_SE_FADEOUT
    (0x7510000d, Some(2)), // u13/0x10 ECT_SOUND_ENV_SE_FADE
    (0x7510000f, Some(1)), // u15/0x10 ECT_SOUND_MS_FORCE_STOP
    (0x75200001, Some(3)), // u1/0x20 ECT_CONTRAST_SET
    (0x75200003, Some(1)), // u3/0x20 ECT_CONTRAST_WAIT
    (0x75200004, Some(3)), // u4/0x20 ECT_CONTRAST_SET_MIX
    (0x75300001, Some(1)), // u1/0x30
    (0x75300002, Some(2)), // u2/0x30
    (0x75300004, Some(1)), // u4/0x30
    (0x75400005, Some(0)), // u5/0x40
    (0x75400006, Some(0)), // u6/0x40
    (0x75400007, Some(0)), // u7/0x40
    (0x75400009, Some(1)), // u9/0x40
    (0x75600001, Some(1)), // u1/0x60
    (0x77000001, Some(1)), // w1
    (0x77000004, Some(1)), // w4
];

/// Atelier Shallie DX's operand counts, measured from its 2,194 events (`barrel ebd-measure`)
/// the way Escha's were; not yet checked against its executable's handlers. Most commands
/// match Escha's, but 58 are Shallie's own and 7 take other counts (`q55` takes 14, the shape
/// of Escha's `q54`), so some numbers mean other things there (docs/events.md, "Shallie DX").
pub static SHALLIE: &[(u32, Option<u8>)] = &[
    (0x70000001, Some(1)), // p1
    (0x70000002, Some(0)), // p2
    (0x70000003, Some(1)), // p3
    (0x70000004, Some(1)), // p4
    (0x70000005, Some(0)), // p5
    (0x71000002, Some(1)), // q2
    (0x71000003, Some(2)), // q3
    (0x71000004, Some(14)), // q4
    (0x71000005, Some(1)), // q5
    (0x71000006, Some(0)), // q6
    (0x71000007, Some(1)), // q7
    (0x71000008, Some(5)), // q8
    (0x7100000a, Some(1)), // q10
    (0x7100000b, Some(0)), // q11
    (0x7100000c, Some(1)), // q12
    (0x7100000d, Some(0)), // q13
    (0x71000010, Some(3)), // q16
    (0x71000011, Some(6)), // q17
    (0x71000012, Some(1)), // q18
    (0x71000013, Some(1)), // q19
    (0x71000019, Some(2)), // q25
    (0x71000022, Some(4)), // q34
    (0x71000023, Some(6)), // q35
    (0x71000026, Some(7)), // q38
    (0x71000028, Some(11)), // q40
    (0x71000029, Some(11)), // q41
    (0x7100002b, Some(6)), // q43
    (0x7100002c, Some(1)), // q44
    (0x7100002d, Some(3)), // q45
    (0x7100002e, Some(0)), // q46
    (0x71000037, Some(14)), // q55
    (0x71000038, Some(1)), // q56
    (0x7100003b, Some(1)), // q59
    (0x7100003c, Some(0)), // q60
    (0x72000004, Some(2)), // r4
    (0x72000008, Some(1)), // r8
    (0x7200000a, Some(0)), // r10
    (0x7200000d, Some(4)), // r13
    (0x7200000e, Some(6)), // r14
    (0x7200000f, Some(0)), // r15
    (0x72000010, Some(14)), // r16
    (0x72000011, Some(2)), // r17
    (0x72000012, Some(7)), // r18
    (0x72000013, Some(1)), // r19
    (0x72000014, Some(2)), // r20
    (0x72000016, Some(6)), // r22
    (0x72000018, Some(4)), // r24
    (0x7200001b, Some(1)), // r27
    (0x7200001c, Some(3)), // r28
    (0x7200001d, Some(1)), // r29
    (0x7200001e, Some(3)), // r30
    (0x7200001f, Some(4)), // r31
    (0x72000020, Some(6)), // r32
    (0x72000021, Some(1)), // r33
    (0x72000022, Some(3)), // r34
    (0x72000023, Some(1)), // r35
    (0x72000024, Some(1)), // r36
    (0x73000001, Some(5)), // s1
    (0x73000002, Some(3)), // s2
    (0x73000003, Some(2)), // s3
    (0x73000004, Some(7)), // s4
    (0x73000005, Some(4)), // s5
    (0x73000006, Some(1)), // s6
    (0x73000008, Some(4)), // s8
    (0x73000009, Some(2)), // s9
    (0x7300000a, Some(4)), // s10
    (0x7300000b, Some(5)), // s11
    (0x7300000c, Some(4)), // s12
    (0x7300000d, Some(3)), // s13
    (0x7300000e, Some(3)), // s14
    (0x7300000f, Some(5)), // s15
    (0x73000010, Some(7)), // s16
    (0x73000011, Some(1)), // s17
    (0x73000012, None), // s18
    (0x73000013, Some(1)), // s19
    (0x73000015, Some(2)), // s21
    (0x73000016, Some(1)), // s22
    (0x73000017, Some(4)), // s23
    (0x73000018, Some(7)), // s24
    (0x73000019, Some(7)), // s25
    (0x7300001a, Some(6)), // s26
    (0x7300001b, Some(9)), // s27
    (0x7300001c, Some(9)), // s28
    (0x7300001d, Some(1)), // s29
    (0x73000021, Some(1)), // s33
    (0x73000022, Some(6)), // s34
    (0x73000023, Some(4)), // s35
    (0x73000024, Some(6)), // s36
    (0x73000025, Some(1)), // s37
    (0x73000026, Some(25)), // s38
    (0x73000027, Some(2)), // s39
    (0x73000029, None), // s41
    (0x7300002a, Some(2)), // s42
    (0x7300002b, Some(2)), // s43
    (0x7300002c, Some(3)), // s44
    (0x7300002d, Some(2)), // s45
    (0x7300002e, Some(2)), // s46
    (0x7300002f, Some(17)), // s47
    (0x73000030, Some(2)), // s48
    (0x73000031, Some(2)), // s49
    (0x73000033, Some(4)), // s51
    (0x73000034, Some(7)), // s52
    (0x73000035, Some(6)), // s53
    (0x73000036, Some(9)), // s54
    (0x73000037, Some(1)), // s55
    (0x73000038, Some(10)), // s56
    (0x73000039, Some(1)), // s57
    (0x7300003e, Some(6)), // s62
    (0x73000040, Some(3)), // s64
    (0x73000044, Some(12)), // s68
    (0x73000045, Some(2)), // s69
    (0x73000046, Some(1)), // s70
    (0x73000047, Some(2)), // s71
    (0x73000048, Some(2)), // s72
    (0x73000049, Some(3)), // s73
    (0x7300004a, Some(4)), // s74
    (0x7300004c, Some(1)), // s76
    (0x7300004e, Some(1)), // s78
    (0x73000050, Some(3)), // s80
    (0x73000051, Some(1)), // s81
    (0x73000054, Some(7)), // s84
    (0x73000055, Some(7)), // s85
    (0x73000058, Some(4)), // s88
    (0x7300005c, Some(3)), // s92
    (0x7300005d, Some(3)), // s93
    (0x7300005e, Some(3)), // s94
    (0x73000062, Some(19)), // s98
    (0x73000063, Some(6)), // s99
    (0x73000066, Some(2)), // s102
    (0x7300006e, Some(1)), // s110
    (0x7300006f, Some(1)), // s111
    (0x73000070, Some(1)), // s112
    (0x73000071, Some(2)), // s113
    (0x73000072, Some(2)), // s114
    (0x73000073, Some(6)), // s115
    (0x73000074, Some(4)), // s116
    (0x73000075, Some(1)), // s117
    (0x73000079, Some(3)), // s121
    (0x7300007a, Some(2)), // s122
    (0x7300007c, Some(20)), // s124
    (0x7300007d, Some(2)), // s125
    (0x74000001, None), // t1
    (0x74000002, Some(1)), // t2
    (0x74000003, Some(6)), // t3
    (0x74000004, None), // t4
    (0x74000005, Some(0)), // t5
    (0x74000006, Some(6)), // t6
    (0x74000007, None), // t7
    (0x74000008, Some(0)), // t8
    (0x74000009, Some(12)), // t9
    (0x7400000a, Some(4)), // t10
    (0x7400000b, Some(0)), // t11
    (0x7400000c, Some(3)), // t12
    (0x7400000d, Some(0)), // t13
    (0x7400000e, Some(2)), // t14
    (0x74000010, Some(0)), // t16
    (0x74000018, Some(1)), // t24
    (0x74000019, Some(0)), // t25
    (0x7400001a, Some(3)), // t26
    (0x7400001d, Some(0)), // t29
    (0x7400001e, Some(1)), // t30
    (0x74100001, Some(1)), // t1/0x10
    (0x74100002, Some(0)), // t2/0x10
    (0x74100003, Some(2)), // t3/0x10
    (0x74100004, Some(2)), // t4/0x10
    (0x74100005, Some(2)), // t5/0x10
    (0x74100007, Some(2)), // t7/0x10
    (0x74100009, Some(2)), // t9/0x10
    (0x7410000b, Some(2)), // t11/0x10
    (0x7410000c, Some(2)), // t12/0x10
    (0x7410000d, Some(4)), // t13/0x10
    (0x7410000e, Some(4)), // t14/0x10
    (0x74100011, Some(2)), // t17/0x10
    (0x74100012, Some(2)), // t18/0x10
    (0x74100013, Some(1)), // t19/0x10
    (0x74100014, Some(11)), // t20/0x10
    (0x74100015, Some(12)), // t21/0x10
    (0x7420000c, Some(0)), // t12/0x20
    (0x74200011, Some(1)), // t17/0x20
    (0x74200012, Some(1)), // t18/0x20
    (0x74200013, Some(0)), // t19/0x20
    (0x74300001, Some(0)), // t1/0x30
    (0x74300002, Some(2)), // t2/0x30
    (0x75000001, Some(0)), // u1
    (0x75000002, Some(1)), // u2
    (0x75000003, Some(1)), // u3
    (0x75000004, Some(0)), // u4
    (0x75000005, Some(1)), // u5
    (0x75000007, Some(1)), // u7
    (0x75000008, Some(3)), // u8
    (0x7500000b, Some(6)), // u11
    (0x7500000c, Some(1)), // u12
    (0x7500000d, None), // u13
    (0x7500000e, Some(0)), // u14
    (0x7500000f, Some(0)), // u15
    (0x75000010, Some(2)), // u16
    (0x75000011, Some(0)), // u17
    (0x75000012, Some(2)), // u18
    (0x75000015, Some(1)), // u21
    (0x75000016, Some(3)), // u22
    (0x7500001a, Some(4)), // u26
    (0x7500001b, Some(5)), // u27
    (0x7500001d, Some(5)), // u29
    (0x7500001e, Some(1)), // u30
    (0x7500001f, Some(1)), // u31
    (0x75000020, Some(2)), // u32
    (0x75000021, Some(0)), // u33
    (0x75000022, Some(2)), // u34
    (0x75000023, Some(2)), // u35
    (0x75000024, Some(2)), // u36
    (0x75000026, Some(1)), // u38
    (0x75000028, Some(1)), // u40
    (0x75000029, Some(1)), // u41
    (0x7500002e, Some(3)), // u46
    (0x7500002f, Some(0)), // u47
    (0x75000030, Some(0)), // u48
    (0x75000032, Some(3)), // u50
    (0x75000033, Some(1)), // u51
    (0x75000034, Some(0)), // u52
    (0x75000035, Some(0)), // u53
    (0x75000037, Some(1)), // u55
    (0x75000038, Some(3)), // u56
    (0x7500003a, Some(0)), // u58
    (0x75000043, Some(1)), // u67
    (0x75000044, Some(0)), // u68
    (0x75000049, Some(1)), // u73
    (0x7500004a, Some(1)), // u74
    (0x7500004f, Some(0)), // u79
    (0x75000061, Some(3)), // u97
    (0x75000067, Some(6)), // u103
    (0x75000082, Some(2)), // u130
    (0x75000086, Some(1)), // u134
    (0x75000087, Some(0)), // u135
    (0x75000088, Some(0)), // u136
    (0x7500008b, Some(0)), // u139
    (0x7500008c, Some(1)), // u140
    (0x75000090, Some(1)), // u144
    (0x75000092, Some(2)), // u146
    (0x75000093, Some(0)), // u147
    (0x75000094, Some(0)), // u148
    (0x75000096, Some(1)), // u150
    (0x75000097, Some(4)), // u151
    (0x75000098, Some(2)), // u152
    (0x75100001, Some(5)), // u1/0x10
    (0x75100002, Some(1)), // u2/0x10
    (0x75100003, Some(4)), // u3/0x10
    (0x75100005, Some(2)), // u5/0x10
    (0x75100006, Some(3)), // u6/0x10
    (0x75100007, Some(3)), // u7/0x10
    (0x75100008, Some(5)), // u8/0x10
    (0x75200001, Some(3)), // u1/0x20
    (0x75200003, Some(1)), // u3/0x20
    (0x75200004, Some(3)), // u4/0x20
    (0x75300001, Some(1)), // u1/0x30
    (0x75300002, Some(2)), // u2/0x30
    (0x75300003, Some(0)), // u3/0x30
    (0x75300004, Some(1)), // u4/0x30
    (0x75300007, Some(7)), // u7/0x30
    (0x75400001, Some(4)), // u1/0x40
    (0x75400002, Some(3)), // u2/0x40
    (0x75400003, Some(2)), // u3/0x40
    (0x75400004, Some(2)), // u4/0x40
    (0x75400005, Some(2)), // u5/0x40
    (0x75400006, Some(0)), // u6/0x40
    (0x75400007, Some(0)), // u7/0x40
    (0x75400008, Some(0)), // u8/0x40
    (0x75400009, Some(0)), // u9/0x40
    (0x7540000a, Some(0)), // u10/0x40
    (0x77000001, Some(1)), // w1
    (0x77000004, Some(1)), // w4
];

/// Atelier Ayesha DX's operand counts, measured from its 3,891 events (`barrel ebd-measure`)
/// the way Escha's were; not yet checked against its executable's handlers.
pub static AYESHA: &[(u32, Option<u8>)] = &[
    (0x70000001, Some(1)), // p1
    (0x70000002, Some(0)), // p2
    (0x70000003, Some(1)), // p3
    (0x70000004, Some(1)), // p4
    (0x70000005, Some(0)), // p5
    (0x71000003, Some(1)), // q3
    (0x71000004, Some(2)), // q4
    (0x71000005, Some(14)), // q5
    (0x71000006, None), // q6
    (0x71000007, Some(1)), // q7
    (0x71000008, Some(0)), // q8
    (0x71000009, Some(1)), // q9
    (0x7100000a, Some(5)), // q10
    (0x7100000c, Some(1)), // q12
    (0x7100000d, Some(0)), // q13
    (0x7100000e, Some(1)), // q14
    (0x7100000f, Some(1)), // q15
    (0x71000010, Some(0)), // q16
    (0x71000012, Some(3)), // q18
    (0x71000016, Some(2)), // q22
    (0x71000017, Some(9)), // q23
    (0x71000018, Some(5)), // q24
    (0x7100001b, Some(2)), // q27
    (0x7100001f, Some(1)), // q31
    (0x71000023, Some(5)), // q35
    (0x71000024, Some(4)), // q36
    (0x71000025, Some(6)), // q37
    (0x71000026, Some(0)), // q38
    (0x71000027, Some(1)), // q39
    (0x71000028, Some(7)), // q40
    (0x71000029, Some(1)), // q41
    (0x7100002a, Some(15)), // q42
    (0x7100002b, Some(15)), // q43
    (0x72000004, Some(2)), // r4
    (0x72000006, Some(2)), // r6
    (0x72000008, Some(1)), // r8
    (0x7200000d, Some(4)), // r13
    (0x7200000e, Some(6)), // r14
    (0x7200000f, Some(0)), // r15
    (0x72000010, Some(14)), // r16
    (0x72000011, Some(2)), // r17
    (0x72000012, Some(7)), // r18
    (0x72000013, Some(1)), // r19
    (0x72000014, Some(2)), // r20
    (0x72000016, Some(6)), // r22
    (0x72000018, Some(4)), // r24
    (0x7200001b, Some(1)), // r27
    (0x7200001c, Some(3)), // r28
    (0x7200001d, Some(1)), // r29
    (0x7200001e, Some(3)), // r30
    (0x73000001, Some(5)), // s1
    (0x73000002, Some(3)), // s2
    (0x73000003, Some(2)), // s3
    (0x73000004, Some(7)), // s4
    (0x73000005, Some(4)), // s5
    (0x73000006, Some(1)), // s6
    (0x73000007, Some(2)), // s7
    (0x73000008, Some(4)), // s8
    (0x73000009, Some(2)), // s9
    (0x7300000a, Some(4)), // s10
    (0x7300000b, Some(5)), // s11
    (0x7300000c, Some(4)), // s12
    (0x7300000d, Some(3)), // s13
    (0x7300000e, Some(3)), // s14
    (0x7300000f, Some(5)), // s15
    (0x73000010, Some(7)), // s16
    (0x73000011, Some(1)), // s17
    (0x73000012, None), // s18
    (0x73000013, Some(1)), // s19
    (0x73000014, Some(13)), // s20
    (0x73000015, Some(2)), // s21
    (0x73000017, Some(4)), // s23
    (0x73000018, Some(7)), // s24
    (0x73000019, Some(7)), // s25
    (0x7300001a, Some(6)), // s26
    (0x7300001b, Some(9)), // s27
    (0x7300001c, Some(9)), // s28
    (0x7300001d, Some(1)), // s29
    (0x7300001f, Some(6)), // s31
    (0x73000021, Some(1)), // s33
    (0x73000022, Some(6)), // s34
    (0x73000023, Some(4)), // s35
    (0x73000024, Some(6)), // s36
    (0x73000025, Some(1)), // s37
    (0x73000026, Some(25)), // s38
    (0x73000027, Some(2)), // s39
    (0x73000029, None), // s41
    (0x7300002a, Some(2)), // s42
    (0x7300002c, Some(3)), // s44
    (0x7300002d, Some(2)), // s45
    (0x7300002e, Some(2)), // s46
    (0x7300002f, Some(17)), // s47
    (0x73000030, Some(2)), // s48
    (0x73000031, Some(2)), // s49
    (0x73000033, Some(4)), // s51
    (0x73000034, Some(7)), // s52
    (0x73000035, Some(6)), // s53
    (0x73000036, Some(9)), // s54
    (0x73000037, Some(1)), // s55
    (0x73000038, Some(10)), // s56
    (0x73000039, Some(1)), // s57
    (0x7300003e, Some(6)), // s62
    (0x73000040, Some(3)), // s64
    (0x73000042, Some(4)), // s66
    (0x73000043, Some(3)), // s67
    (0x73000044, Some(12)), // s68
    (0x73000045, Some(2)), // s69
    (0x73000046, Some(1)), // s70
    (0x73000047, Some(2)), // s71
    (0x73000050, Some(3)), // s80
    (0x73000051, Some(1)), // s81
    (0x73000052, Some(4)), // s82
    (0x73000053, Some(1)), // s83
    (0x73000054, Some(7)), // s84
    (0x73000055, Some(7)), // s85
    (0x73000056, Some(9)), // s86
    (0x73000058, Some(4)), // s88
    (0x7300005b, Some(2)), // s91
    (0x7300005d, Some(3)), // s93
    (0x7300005e, Some(3)), // s94
    (0x7300005f, Some(2)), // s95
    (0x73000061, Some(19)), // s97
    (0x73000062, Some(19)), // s98
    (0x73000063, Some(6)), // s99
    (0x73000064, Some(6)), // s100
    (0x73000066, Some(2)), // s102
    (0x73000068, Some(2)), // s104
    (0x7300006a, Some(4)), // s106
    (0x7300006e, Some(1)), // s110
    (0x7300006f, Some(1)), // s111
    (0x73000070, Some(1)), // s112
    (0x74000001, None), // t1
    (0x74000002, Some(1)), // t2
    (0x74000003, Some(6)), // t3
    (0x74000004, None), // t4
    (0x74000005, Some(0)), // t5
    (0x74000006, Some(6)), // t6
    (0x74000007, None), // t7
    (0x74000008, Some(0)), // t8
    (0x74000009, Some(12)), // t9
    (0x7400000a, Some(4)), // t10
    (0x7400000b, Some(0)), // t11
    (0x7400000c, Some(3)), // t12
    (0x7400000d, Some(0)), // t13
    (0x7400000e, Some(2)), // t14
    (0x74000010, Some(0)), // t16
    (0x74000018, Some(1)), // t24
    (0x74000019, Some(0)), // t25
    (0x7400001a, Some(3)), // t26
    (0x7400001d, Some(0)), // t29
    (0x7400001e, Some(1)), // t30
    (0x74100002, Some(0)), // t2/0x10
    (0x74100003, Some(2)), // t3/0x10
    (0x74100005, Some(2)), // t5/0x10
    (0x74100007, Some(2)), // t7/0x10
    (0x74100009, Some(2)), // t9/0x10
    (0x7410000b, Some(2)), // t11/0x10
    (0x7410000c, Some(2)), // t12/0x10
    (0x7410000d, Some(4)), // t13/0x10
    (0x7410000f, Some(4)), // t15/0x10
    (0x74100012, Some(2)), // t18/0x10
    (0x74100013, Some(1)), // t19/0x10
    (0x74100014, Some(11)), // t20/0x10
    (0x74100015, Some(12)), // t21/0x10
    (0x7420000c, Some(0)), // t12/0x20
    (0x74200011, Some(1)), // t17/0x20
    (0x74200012, Some(1)), // t18/0x20
    (0x74200013, Some(0)), // t19/0x20
    (0x74300001, Some(0)), // t1/0x30
    (0x74300002, Some(2)), // t2/0x30
    (0x75000001, Some(0)), // u1
    (0x75000002, Some(1)), // u2
    (0x75000003, Some(1)), // u3
    (0x75000005, Some(1)), // u5
    (0x75000007, Some(1)), // u7
    (0x75000008, Some(3)), // u8
    (0x7500000b, Some(6)), // u11
    (0x7500000c, Some(1)), // u12
    (0x7500000d, None), // u13
    (0x7500000e, Some(0)), // u14
    (0x7500000f, Some(0)), // u15
    (0x75000010, Some(2)), // u16
    (0x75000011, Some(0)), // u17
    (0x75000012, Some(2)), // u18
    (0x75000015, Some(1)), // u21
    (0x75000016, Some(3)), // u22
    (0x75000019, Some(3)), // u25
    (0x7500001a, Some(4)), // u26
    (0x7500001b, Some(5)), // u27
    (0x7500001d, Some(5)), // u29
    (0x7500001e, Some(1)), // u30
    (0x7500001f, Some(1)), // u31
    (0x75000022, Some(2)), // u34
    (0x75000023, Some(2)), // u35
    (0x75000024, Some(2)), // u36
    (0x75000026, Some(1)), // u38
    (0x75000027, Some(4)), // u39
    (0x75000029, Some(1)), // u41
    (0x7500002e, Some(3)), // u46
    (0x7500002f, Some(0)), // u47
    (0x75000030, Some(0)), // u48
    (0x75000033, Some(1)), // u51
    (0x75000034, Some(0)), // u52
    (0x75000035, Some(0)), // u53
    (0x75000037, Some(1)), // u55
    (0x75000038, Some(3)), // u56
    (0x75000043, Some(1)), // u67
    (0x75000044, Some(0)), // u68
    (0x75000045, Some(3)), // u69
    (0x75000058, Some(1)), // u88
    (0x75000061, Some(3)), // u97
    (0x75000067, Some(6)), // u103
    (0x75000068, Some(0)), // u104
    (0x75000069, Some(0)), // u105
    (0x7500006a, Some(0)), // u106
    (0x7500006d, Some(2)), // u109
    (0x7500006e, Some(0)), // u110
    (0x7500006f, Some(0)), // u111
    (0x75000070, Some(0)), // u112
    (0x75000072, Some(0)), // u114
    (0x75000073, Some(1)), // u115
    (0x75000074, Some(1)), // u116
    (0x75000075, Some(1)), // u117
    (0x75000076, Some(4)), // u118
    (0x75000077, Some(1)), // u119
    (0x75000082, Some(2)), // u130
    (0x75000086, Some(1)), // u134
    (0x75000087, Some(0)), // u135
    (0x75000088, Some(0)), // u136
    (0x75000089, Some(2)), // u137
    (0x7500008a, Some(0)), // u138
    (0x7500008b, Some(0)), // u139
    (0x7500008c, Some(1)), // u140
    (0x7500008d, Some(0)), // u141
    (0x7500008e, Some(0)), // u142
    (0x7500008f, Some(0)), // u143
    (0x75000090, Some(1)), // u144
    (0x75000091, Some(7)), // u145
    (0x75100001, Some(5)), // u1/0x10
    (0x75100002, Some(1)), // u2/0x10
    (0x75100003, Some(4)), // u3/0x10
    (0x75100004, Some(1)), // u4/0x10
    (0x75100005, Some(2)), // u5/0x10
    (0x75100008, Some(5)), // u8/0x10
    (0x75200001, Some(3)), // u1/0x20
    (0x75200002, Some(1)), // u2/0x20
    (0x75200003, Some(1)), // u3/0x20
    (0x75200004, Some(3)), // u4/0x20
    (0x75300001, Some(1)), // u1/0x30
    (0x75300002, Some(2)), // u2/0x30
    (0x75300003, Some(0)), // u3/0x30
    (0x75300004, Some(1)), // u4/0x30
    (0x75300007, Some(7)), // u7/0x30
    (0x75300008, Some(0)), // u8/0x30
    (0x75350001, Some(2)), // u1/0x35
    (0x75400001, Some(4)), // u1/0x40
    (0x75400002, Some(3)), // u2/0x40
    (0x75400003, Some(2)), // u3/0x40
    (0x75400009, Some(0)), // u9/0x40
    (0x7540000a, Some(0)), // u10/0x40
    (0x77000001, Some(1)), // w1
    (0x77000004, Some(1)), // w4
];
