use super::*;

pub(super) const LONGEST_NAME: u32 = 30;

pub(super) static NAMES: &str = "NoSymbolspaceexclamquotedblnumbersigndollarpercentampersandapostrophequoterightparenleftparenrightasteriskpluscommaminusperiodslash0123456789colonsemicolonlessequalgreaterquestionatABCDEFGHIJKLMNOPQRSTUVWXYZbracketleftbackslashbracketrightasciicircumunderscoregravequoteleftabcdefghijklmnopqrstuvwxyzbraceleftbarbracerightasciitildenobreakspaceexclamdowncentsterlingcurrencyyenbrokenbarsectiondiaeresiscopyrightordfeminineguillemotleftguillemetleftnotsignhyphenregisteredmacrondegreeplusminustwosuperiorthreesuperioracutemuparagraphperiodcenteredcedillaonesuperiormasculineordmasculineguillemotrightguillemetrightonequarteronehalfthreequartersquestiondownAgraveAacuteAcircumflexAtildeAdiaeresisAringAECcedillaEgraveEacuteEcircumflexEdiaeresisIgraveIacuteIcircumflexIdiaeresisETHEthNtildeOgraveOacuteOcircumflexOtildeOdiaeresismultiplyOslashOobliqueUgraveUacuteUcircumflexUdiaeresisYacuteTHORNThornssharpagraveaacuteacircumflexatildeadiaeresisaringaeccedillaegraveeacuteecircumflexediaeresisigraveiacuteicircumflexidiaeresisethntildeograveoacuteocircumflexotildeodiaeresisdivisionoslashoobliqueugraveuacuteucircumflexudiaeresisyacutethornydiaeresisAogonekbreveLstrokeLcaronSacuteScaronScedillaTcaronZacuteZcaronZabovedotaogonekogoneklstrokelcaronsacutecaronscaronscedillatcaronzacutedoubleacutezcaronzabovedotRacuteAbreveLacuteCacuteCcaronEogonekEcaronDcaronDstrokeNacuteNcaronOdoubleacuteRcaronUringUdoubleacuteTcedillaracuteabrevelacutecacuteccaroneogonekecarondcarondstrokenacutencaronodoubleacutercaronuringudoubleacutetcedillaabovedotHstrokeHcircumflexIabovedotGbreveJcircumflexhstrokehcircumflexidotlessgbrevejcircumflexCabovedotCcircumflexGabovedotGcircumflexUbreveScircumflexcabovedotccircumflexgabovedotgcircumflexubrevescircumflexkrakappaRcedillaItildeLcedillaEmacronGcedillaTslashrcedillaitildelcedillaemacrongcedillatslashENGengAmacronIogonekEabovedotImacronNcedillaOmacronKcedillaUogonekUtildeUmacronamacroniogonekeabovedotimacronncedillaomacronkcedillauogonekutildeumacronoverlinekana_fullstopkana_openingbracketkana_closingbracketkana_commakana_conjunctivekana_middledotkana_WOkana_akana_ikana_ukana_ekana_okana_yakana_yukana_yokana_tsukana_tuprolongedsoundkana_Akana_Ikana_Ukana_Ekana_Okana_KAkana_KIkana_KUkana_KEkana_KOkana_SAkana_SHIkana_SUkana_SEkana_SOkana_TAkana_CHIkana_TIkana_TSUkana_TUkana_TEkana_TOkana_NAkana_NIkana_NUkana_NEkana_NOkana_HAkana_HIkana_FUkana_HUkana_HEkana_HOkana_MAkana_MIkana_MUkana_MEkana_MOkana_YAkana_YUkana_YOkana_RAkana_RIkana_RUkana_REkana_ROkana_WAkana_NvoicedsoundsemivoicedsoundArabic_commaArabic_semicolonArabic_question_markArabic_hamzaArabic_maddaonalefArabic_hamzaonalefArabic_hamzaonwawArabic_hamzaunderalefArabic_hamzaonyehArabic_alefArabic_behArabic_tehmarbutaArabic_tehArabic_thehArabic_jeemArabic_hahArabic_khahArabic_dalArabic_thalArabic_raArabic_zainArabic_seenArabic_sheenArabic_sadArabic_dadArabic_tahArabic_zahArabic_ainArabic_ghainArabic_tatweelArabic_fehArabic_qafArabic_kafArabic_lamArabic_meemArabic_noonArabic_haArabic_hehArabic_wawArabic_alefmaksuraArabic_yehArabic_fathatanArabic_dammatanArabic_kasratanArabic_fathaArabic_dammaArabic_kasraArabic_shaddaArabic_sukunSerbian_djeMacedonia_gjeCyrillic_ioUkrainian_ieUkranian_jeMacedonia_dseUkrainian_iUkranian_iUkrainian_yiUkranian_yiCyrillic_jeSerbian_jeCyrillic_ljeSerbian_ljeCyrillic_njeSerbian_njeSerbian_tsheMacedonia_kjeUkrainian_ghe_with_upturnByelorussian_shortuCyrillic_dzheSerbian_dzenumerosignSerbian_DJEMacedonia_GJECyrillic_IOUkrainian_IEUkranian_JEMacedonia_DSEUkrainian_IUkranian_IUkrainian_YIUkranian_YICyrillic_JESerbian_JECyrillic_LJESerbian_LJECyrillic_NJESerbian_NJESerbian_TSHEMacedonia_KJEUkrainian_GHE_WITH_UPTURNByelorussian_SHORTUCyrillic_DZHESerbian_DZECyrillic_yuCyrillic_aCyrillic_beCyrillic_tseCyrillic_deCyrillic_ieCyrillic_efCyrillic_gheCyrillic_haCyrillic_iCyrillic_shortiCyrillic_kaCyrillic_elCyrillic_emCyrillic_enCyrillic_oCyrillic_peCyrillic_yaCyrillic_erCyrillic_esCyrillic_teCyrillic_uCyrillic_zheCyrillic_veCyrillic_softsignCyrillic_yeruCyrillic_zeCyrillic_shaCyrillic_eCyrillic_shchaCyrillic_cheCyrillic_hardsignCyrillic_YUCyrillic_ACyrillic_BECyrillic_TSECyrillic_DECyrillic_IECyrillic_EFCyrillic_GHECyrillic_HACyrillic_ICyrillic_SHORTICyrillic_KACyrillic_ELCyrillic_EMCyrillic_ENCyrillic_OCyrillic_PECyrillic_YACyrillic_ERCyrillic_ESCyrillic_TECyrillic_UCyrillic_ZHECyrillic_VECyrillic_SOFTSIGNCyrillic_YERUCyrillic_ZECyrillic_SHACyrillic_ECyrillic_SHCHACyrillic_CHECyrillic_HARDSIGNGreek_ALPHAaccentGreek_EPSILONaccentGreek_ETAaccentGreek_IOTAaccentGreek_IOTAdieresisGreek_IOTAdiaeresisGreek_OMICRONaccentGreek_UPSILONaccentGreek_UPSILONdieresisGreek_OMEGAaccentGreek_accentdieresisGreek_horizbarGreek_alphaaccentGreek_epsilonaccentGreek_etaaccentGreek_iotaaccentGreek_iotadieresisGreek_iotaaccentdieresisGreek_omicronaccentGreek_upsilonaccentGreek_upsilondieresisGreek_upsilonaccentdieresisGreek_omegaaccentGreek_ALPHAGreek_BETAGreek_GAMMAGreek_DELTAGreek_EPSILONGreek_ZETAGreek_ETAGreek_THETAGreek_IOTAGreek_KAPPAGreek_LAMDAGreek_LAMBDAGreek_MUGreek_NUGreek_XIGreek_OMICRONGreek_PIGreek_RHOGreek_SIGMAGreek_TAUGreek_UPSILONGreek_PHIGreek_CHIGreek_PSIGreek_OMEGAGreek_alphaGreek_betaGreek_gammaGreek_deltaGreek_epsilonGreek_zetaGreek_etaGreek_thetaGreek_iotaGreek_kappaGreek_lamdaGreek_lambdaGreek_muGreek_nuGreek_xiGreek_omicronGreek_piGreek_rhoGreek_sigmaGreek_finalsmallsigmaGreek_tauGreek_upsilonGreek_phiGreek_chiGreek_psiGreek_omegaleftradicaltopleftradicalhorizconnectortopintegralbotintegralvertconnectortopleftsqbracketbotleftsqbrackettoprightsqbracketbotrightsqbrackettopleftparensbotleftparenstoprightparensbotrightparensleftmiddlecurlybracerightmiddlecurlybracetopleftsummationbotleftsummationtopvertsummationconnectorbotvertsummationconnectortoprightsummationbotrightsummationrightmiddlesummationlessthanequalnotequalgreaterthanequalintegralthereforevariationinfinitynablaapproximatesimilarequalifonlyifimpliesidenticalradicalincludedinincludesintersectionunionlogicalandlogicalorpartialderivativefunctionleftarrowuparrowrightarrowdownarrowblanksoliddiamondcheckerboardhtffcrlfnlvtlowrightcorneruprightcornerupleftcornerlowleftcornercrossinglineshorizlinescan1horizlinescan3horizlinescan5horizlinescan7horizlinescan9lefttrighttbotttoptvertbaremspaceenspaceem3spaceem4spacedigitspacepunctspacethinspacehairspaceemdashendashsignifblankellipsisdoubbaselinedotonethirdtwothirdsonefifthtwofifthsthreefifthsfourfifthsonesixthfivesixthscareoffigdashleftanglebracketdecimalpointrightanglebracketmarkeroneeighththreeeighthsfiveeighthsseveneighthstrademarksignaturemarktrademarkincircleleftopentrianglerightopentriangleemopencircleemopenrectangleleftsinglequotemarkrightsinglequotemarkleftdoublequotemarkrightdoublequotemarkprescriptionpermilleminutessecondslatincrosshexagramfilledrectbulletfilledlefttribulletfilledrighttribulletemfilledcircleemfilledrectenopencircbulletenopensquarebulletopenrectbulletopentribulletupopentribulletdownopenstarenfilledcircbulletenfilledsqbulletfilledtribulletupfilledtribulletdownleftpointerrightpointerclubdiamondheartmaltesecrossdaggerdoubledaggercheckmarkballotcrossmusicalsharpmusicalflatmalesymbolfemalesymboltelephonetelephonerecorderphonographcopyrightcaretsinglelowquotemarkdoublelowquotemarkcursorleftcaretrightcaretdowncaretupcaretoverbardowntackupshoedownstileunderbarjotquaduptackcircleupstiledownshoerightshoeleftshoelefttackrighttackhebrew_doublelowlinehebrew_alephhebrew_bethebrew_bethhebrew_gimelhebrew_gimmelhebrew_dalethebrew_dalethhebrew_hehebrew_wawhebrew_zainhebrew_zayinhebrew_chethebrew_hethebrew_tethebrew_tethhebrew_yodhebrew_finalkaphhebrew_kaphhebrew_lamedhebrew_finalmemhebrew_memhebrew_finalnunhebrew_nunhebrew_samechhebrew_samekhhebrew_ayinhebrew_finalpehebrew_pehebrew_finalzadehebrew_finalzadihebrew_zadehebrew_zadihebrew_qophhebrew_kufhebrew_reshhebrew_shinhebrew_tawhebrew_tafThai_kokaiThai_khokhaiThai_khokhuatThai_khokhwaiThai_khokhonThai_khorakhangThai_ngonguThai_chochanThai_chochingThai_chochangThai_sosoThai_chochoeThai_yoyingThai_dochadaThai_topatakThai_thothanThai_thonangmonthoThai_thophuthaoThai_nonenThai_dodekThai_totaoThai_thothungThai_thothahanThai_thothongThai_nonuThai_bobaimaiThai_poplaThai_phophungThai_fofaThai_phophanThai_fofanThai_phosamphaoThai_momaThai_yoyakThai_roruaThai_ruThai_lolingThai_luThai_wowaenThai_sosalaThai_sorusiThai_sosuaThai_hohipThai_lochulaThai_oangThai_honokhukThai_paiyannoiThai_saraaThai_maihanakatThai_saraaaThai_saraamThai_saraiThai_saraiiThai_saraueThai_saraueeThai_sarauThai_sarauuThai_phinthuThai_maihanakat_maithoThai_bahtThai_saraeThai_saraaeThai_saraoThai_saraaimaimuanThai_saraaimaimalaiThai_lakkhangyaoThai_maiyamokThai_maitaikhuThai_maiekThai_maithoThai_maitriThai_maichattawaThai_thanthakhatThai_nikhahitThai_leksunThai_leknungThai_leksongThai_leksamThai_leksiThai_lekhaThai_lekhokThai_lekchetThai_lekpaetThai_lekkaoHangul_KiyeogHangul_SsangKiyeogHangul_KiyeogSiosHangul_NieunHangul_NieunJieujHangul_NieunHieuhHangul_DikeudHangul_SsangDikeudHangul_RieulHangul_RieulKiyeogHangul_RieulMieumHangul_RieulPieubHangul_RieulSiosHangul_RieulTieutHangul_RieulPhieufHangul_RieulHieuhHangul_MieumHangul_PieubHangul_SsangPieubHangul_PieubSiosHangul_SiosHangul_SsangSiosHangul_IeungHangul_JieujHangul_SsangJieujHangul_CieucHangul_KhieuqHangul_TieutHangul_PhieufHangul_HieuhHangul_AHangul_AEHangul_YAHangul_YAEHangul_EOHangul_EHangul_YEOHangul_YEHangul_OHangul_WAHangul_WAEHangul_OEHangul_YOHangul_UHangul_WEOHangul_WEHangul_WIHangul_YUHangul_EUHangul_YIHangul_IHangul_J_KiyeogHangul_J_SsangKiyeogHangul_J_KiyeogSiosHangul_J_NieunHangul_J_NieunJieujHangul_J_NieunHieuhHangul_J_DikeudHangul_J_RieulHangul_J_RieulKiyeogHangul_J_RieulMieumHangul_J_RieulPieubHangul_J_RieulSiosHangul_J_RieulTieutHangul_J_RieulPhieufHangul_J_RieulHieuhHangul_J_MieumHangul_J_PieubHangul_J_PieubSiosHangul_J_SiosHangul_J_SsangSiosHangul_J_IeungHangul_J_JieujHangul_J_CieucHangul_J_KhieuqHangul_J_TieutHangul_J_PhieufHangul_J_HieuhHangul_RieulYeorinHieuhHangul_SunkyeongeumMieumHangul_SunkyeongeumPieubHangul_PanSiosHangul_KkogjiDalrinIeungHangul_SunkyeongeumPhieufHangul_YeorinHieuhHangul_AraeAHangul_AraeAEHangul_J_PanSiosHangul_J_KkogjiDalrinIeungHangul_J_YeorinHieuhKorean_WonOEoeYdiaeresisEuroSign3270_Duplicate3270_FieldMark3270_Right23270_Left23270_BackTab3270_EraseEOF3270_EraseInput3270_Reset3270_Quit3270_PA13270_PA23270_PA33270_Test3270_Attn3270_CursorBlink3270_AltCursor3270_KeyClick3270_Jump3270_Ident3270_Rule3270_Copy3270_Play3270_Setup3270_Record3270_ChangeScreen3270_DeleteWord3270_ExSelect3270_CursorSelect3270_PrintScreen3270_EnterISO_LockISO_Level2_LatchISO_Level3_ShiftISO_Level3_LatchISO_Level3_LockISO_Group_LatchISO_Group_LockISO_Next_GroupISO_Next_Group_LockISO_Prev_GroupISO_Prev_Group_LockISO_First_GroupISO_First_Group_LockISO_Last_GroupISO_Last_Group_LockISO_Level5_ShiftISO_Level5_LatchISO_Level5_LockISO_Left_TabISO_Move_Line_UpISO_Move_Line_DownISO_Partial_Line_UpISO_Partial_Line_DownISO_Partial_Space_LeftISO_Partial_Space_RightISO_Set_Margin_LeftISO_Set_Margin_RightISO_Release_Margin_LeftISO_Release_Margin_RightISO_Release_Both_MarginsISO_Fast_Cursor_LeftISO_Fast_Cursor_RightISO_Fast_Cursor_UpISO_Fast_Cursor_DownISO_Continuous_UnderlineISO_Discontinuous_UnderlineISO_EmphasizeISO_Center_ObjectISO_Enterdead_gravedead_acutedead_circumflexdead_tildedead_perispomenidead_macrondead_brevedead_abovedotdead_diaeresisdead_aboveringdead_doubleacutedead_carondead_cedilladead_ogonekdead_iotadead_voiced_sounddead_semivoiced_sounddead_belowdotdead_hookdead_horndead_strokedead_abovecommadead_psilidead_abovereversedcommadead_dasiadead_doublegravedead_belowringdead_belowmacrondead_belowcircumflexdead_belowtildedead_belowbrevedead_belowdiaeresisdead_invertedbrevedead_belowcommadead_currencyAccessX_EnableAccessX_Feedback_EnableRepeatKeys_EnableSlowKeys_EnableBounceKeys_EnableStickyKeys_EnableMouseKeys_EnableMouseKeys_Accel_EnableOverlay1_EnableOverlay2_EnableAudibleBell_Enabledead_adead_Adead_edead_Edead_idead_Idead_odead_Odead_udead_Udead_small_schwadead_schwadead_capital_schwadead_SCHWAdead_greekdead_hamzadead_apostrophedead_lowlinedead_aboveverticallinedead_belowverticallinedead_longsolidusoverlaychChCHc_hC_hC_HFirst_Virtual_ScreenPrev_Virtual_ScreenNext_Virtual_ScreenLast_Virtual_ScreenTerminate_ServerPointer_LeftPointer_RightPointer_UpPointer_DownPointer_UpLeftPointer_UpRightPointer_DownLeftPointer_DownRightPointer_Button_DfltPointer_Button1Pointer_Button2Pointer_Button3Pointer_Button4Pointer_Button5Pointer_DblClick_DfltPointer_DblClick1Pointer_DblClick2Pointer_DblClick3Pointer_DblClick4Pointer_DblClick5Pointer_Drag_DfltPointer_Drag1Pointer_Drag2Pointer_Drag3Pointer_Drag4Pointer_EnableKeysPointer_AcceleratePointer_DfltBtnNextPointer_DfltBtnPrevPointer_Drag5BackSpaceTabLinefeedClearReturnPauseScroll_LockSys_ReqEscapeMulti_keySunComposeKanjiMuhenkanHenkan_ModeHenkanRomajiHiraganaKatakanaHiragana_KatakanaZenkakuHankakuZenkaku_HankakuTourokuMassyoKana_LockKana_ShiftEisu_ShiftEisu_toggleHangulHangul_StartHangul_EndHangul_HanjaHangul_JamoHangul_RomajaCodeinputKanji_BangouHangul_CodeinputHangul_JeonjaHangul_BanjaHangul_PreHanjaHangul_PostHanjaSingleCandidateHangul_SingleCandidateMultipleCandidateZen_KohoHangul_MultipleCandidatePreviousCandidateMae_KohoHangul_PreviousCandidateHangul_SpecialHomeLeftUpRightDownPriorPage_UpSunPageUpNextPage_DownSunPageDownEndBeginSelectPrintSunPrint_ScreenExecuteInsertUndoSunUndoRedoSunAgainMenuFindSunFindCancelSunStopHelpBreakISO_Group_ShiftMode_switchscript_switchkana_switchArabic_switchGreek_switchHebrew_switchHangul_switchSunAltGraphNum_LockKP_SpaceKP_TabKP_EnterKP_F1KP_F2KP_F3KP_F4KP_HomeKP_LeftKP_UpKP_RightKP_DownKP_PriorKP_Page_UpKP_NextKP_Page_DownKP_EndKP_BeginKP_InsertKP_DeleteKP_MultiplyKP_AddKP_SeparatorKP_SubtractKP_DecimalKP_DivideKP_0KP_1KP_2KP_3KP_4KP_5KP_6KP_7KP_8KP_9KP_EqualF1F2F3F4F5F6F7F8F9F10F11L1F12L2F13L3F14L4F15L5F16L6F17L7F18L8F19L9F20L10F21R1F22R2F23R3F24R4F25R5F26R6F27R7F28R8F29R9F30R10F31R11F32R12F33R13F34R14F35R15Shift_LShift_RControl_LControl_RCaps_LockShift_LockMeta_LMeta_RAlt_LAlt_RSuper_LSuper_RHyper_LHyper_Rbraille_dot_1braille_dot_2braille_dot_3braille_dot_4braille_dot_5braille_dot_6braille_dot_7braille_dot_8braille_dot_9braille_dot_10DeleteVoidSymbolIbreveibreveWcircumflexwcircumflexYcircumflexycircumflexSCHWAObarredOhornohornUhornuhornZstrokezstrokeEZHOcaronocaronGcarongcaronschwaobarredezhcombining_gravecombining_acutecombining_tildecombining_hookcombining_belowdotCyrillic_GHE_barCyrillic_ghe_barCyrillic_ZHE_descenderCyrillic_zhe_descenderCyrillic_KA_descenderCyrillic_ka_descenderCyrillic_KA_vertstrokeCyrillic_ka_vertstrokeCyrillic_EN_descenderCyrillic_en_descenderCyrillic_U_straightCyrillic_u_straightCyrillic_U_straight_barCyrillic_u_straight_barCyrillic_HA_descenderCyrillic_ha_descenderCyrillic_CHE_descenderCyrillic_che_descenderCyrillic_CHE_vertstrokeCyrillic_che_vertstrokeCyrillic_SHHACyrillic_shhaCyrillic_SCHWACyrillic_schwaCyrillic_I_macronCyrillic_i_macronCyrillic_O_barCyrillic_o_barCyrillic_U_macronCyrillic_u_macronArmenian_AYBArmenian_BENArmenian_GIMArmenian_DAArmenian_YECHArmenian_ZAArmenian_EArmenian_ATArmenian_TOArmenian_ZHEArmenian_INIArmenian_LYUNArmenian_KHEArmenian_TSAArmenian_KENArmenian_HOArmenian_DZAArmenian_GHATArmenian_TCHEArmenian_MENArmenian_HIArmenian_NUArmenian_SHAArmenian_VOArmenian_CHAArmenian_PEArmenian_JEArmenian_RAArmenian_SEArmenian_VEVArmenian_TYUNArmenian_REArmenian_TSOArmenian_VYUNArmenian_PYURArmenian_KEArmenian_OArmenian_FEArmenian_apostropheArmenian_accentArmenian_sheshtArmenian_exclamArmenian_amanakArmenian_separation_markArmenian_butArmenian_questionArmenian_paruykArmenian_aybArmenian_benArmenian_gimArmenian_daArmenian_yechArmenian_zaArmenian_eArmenian_atArmenian_toArmenian_zheArmenian_iniArmenian_lyunArmenian_kheArmenian_tsaArmenian_kenArmenian_hoArmenian_dzaArmenian_ghatArmenian_tcheArmenian_menArmenian_hiArmenian_nuArmenian_shaArmenian_voArmenian_chaArmenian_peArmenian_jeArmenian_raArmenian_seArmenian_vevArmenian_tyunArmenian_reArmenian_tsoArmenian_vyunArmenian_pyurArmenian_keArmenian_oArmenian_feArmenian_ligature_ewArmenian_full_stopArmenian_verjaketArmenian_hyphenArmenian_yentamnaArabic_madda_aboveArabic_hamza_aboveArabic_hamza_belowArabic_0Arabic_1Arabic_2Arabic_3Arabic_4Arabic_5Arabic_6Arabic_7Arabic_8Arabic_9Arabic_percentArabic_superscript_alefArabic_ttehArabic_pehArabic_tchehArabic_ddalArabic_rrehArabic_jehArabic_vehArabic_kehehArabic_gafArabic_noon_ghunnaArabic_heh_doachashmeeArabic_heh_goalFarsi_yehArabic_farsi_yehArabic_yeh_bareeArabic_fullstopFarsi_0Farsi_1Farsi_2Farsi_3Farsi_4Farsi_5Farsi_6Farsi_7Farsi_8Farsi_9Sinh_ngSinh_h2Sinh_aSinh_aaSinh_aeSinh_aeeSinh_iSinh_iiSinh_uSinh_uuSinh_riSinh_riiSinh_luSinh_luuSinh_eSinh_eeSinh_aiSinh_oSinh_ooSinh_auSinh_kaSinh_khaSinh_gaSinh_ghaSinh_ng2Sinh_ngaSinh_caSinh_chaSinh_jaSinh_jhaSinh_nyaSinh_jnyaSinh_njaSinh_ttaSinh_tthaSinh_ddaSinh_ddhaSinh_nnaSinh_nddaSinh_thaSinh_thhaSinh_dhaSinh_dhhaSinh_naSinh_ndhaSinh_paSinh_phaSinh_baSinh_bhaSinh_maSinh_mbaSinh_yaSinh_raSinh_laSinh_vaSinh_shaSinh_sshaSinh_saSinh_haSinh_llaSinh_faSinh_alSinh_aa2Sinh_ae2Sinh_aee2Sinh_i2Sinh_ii2Sinh_u2Sinh_uu2Sinh_ru2Sinh_e2Sinh_ee2Sinh_ai2Sinh_o2Sinh_oo2Sinh_au2Sinh_lu2Sinh_ruu2Sinh_luu2Sinh_kunddaliyaGeorgian_anGeorgian_banGeorgian_ganGeorgian_donGeorgian_enGeorgian_vinGeorgian_zenGeorgian_tanGeorgian_inGeorgian_kanGeorgian_lasGeorgian_manGeorgian_narGeorgian_onGeorgian_parGeorgian_zharGeorgian_raeGeorgian_sanGeorgian_tarGeorgian_unGeorgian_pharGeorgian_kharGeorgian_ghanGeorgian_qarGeorgian_shinGeorgian_chinGeorgian_canGeorgian_jilGeorgian_cilGeorgian_charGeorgian_xanGeorgian_jhanGeorgian_haeGeorgian_heGeorgian_hieGeorgian_weGeorgian_harGeorgian_hoeGeorgian_fiBabovedotbabovedotDabovedotdabovedotFabovedotfabovedotLbelowdotlbelowdotMabovedotmabovedotPabovedotpabovedotSabovedotsabovedotTabovedottabovedotWgravewgraveWacutewacuteWdiaeresiswdiaeresisXabovedotxabovedotSSHARPAbelowdotabelowdotAhookahookAcircumflexacuteacircumflexacuteAcircumflexgraveacircumflexgraveAcircumflexhookacircumflexhookAcircumflextildeacircumflextildeAcircumflexbelowdotacircumflexbelowdotAbreveacuteabreveacuteAbrevegraveabrevegraveAbrevehookabrevehookAbrevetildeabrevetildeAbrevebelowdotabrevebelowdotEbelowdotebelowdotEhookehookEtildeetildeEcircumflexacuteecircumflexacuteEcircumflexgraveecircumflexgraveEcircumflexhookecircumflexhookEcircumflextildeecircumflextildeEcircumflexbelowdotecircumflexbelowdotIhookihookIbelowdotibelowdotObelowdotobelowdotOhookohookOcircumflexacuteocircumflexacuteOcircumflexgraveocircumflexgraveOcircumflexhookocircumflexhookOcircumflextildeocircumflextildeOcircumflexbelowdotocircumflexbelowdotOhornacuteohornacuteOhorngraveohorngraveOhornhookohornhookOhorntildeohorntildeOhornbelowdotohornbelowdotUbelowdotubelowdotUhookuhookUhornacuteuhornacuteUhorngraveuhorngraveUhornhookuhornhookUhorntildeuhorntildeUhornbelowdotuhornbelowdotYgraveygraveYbelowdotybelowdotYhookyhookYtildeytildeleftsingleanglequotemarkrightsingleanglequotemarkzerosuperiorfoursuperiorfivesuperiorsixsuperiorsevensuperioreightsuperiorninesuperiorzerosubscriptonesubscripttwosubscriptthreesubscriptfoursubscriptfivesubscriptsixsubscriptsevensubscripteightsubscriptninesubscriptEcuSignColonSignCruzeiroSignFFrancSignLiraSignMillSignNairaSignPesetaSignRupeeSignWonSignNewSheqelSignDongSignpartdifferentialemptysetelementofnotelementofcontainsassquarerootcuberootfourthrootdintegraltintegralbecausenotapproxeqapproxeqnotidenticalstricteqbraille_blankbraille_dots_1braille_dots_2braille_dots_12braille_dots_3braille_dots_13braille_dots_23braille_dots_123braille_dots_4braille_dots_14braille_dots_24braille_dots_124braille_dots_34braille_dots_134braille_dots_234braille_dots_1234braille_dots_5braille_dots_15braille_dots_25braille_dots_125braille_dots_35braille_dots_135braille_dots_235braille_dots_1235braille_dots_45braille_dots_145braille_dots_245braille_dots_1245braille_dots_345braille_dots_1345braille_dots_2345braille_dots_12345braille_dots_6braille_dots_16braille_dots_26braille_dots_126braille_dots_36braille_dots_136braille_dots_236braille_dots_1236braille_dots_46braille_dots_146braille_dots_246braille_dots_1246braille_dots_346braille_dots_1346braille_dots_2346braille_dots_12346braille_dots_56braille_dots_156braille_dots_256braille_dots_1256braille_dots_356braille_dots_1356braille_dots_2356braille_dots_12356braille_dots_456braille_dots_1456braille_dots_2456braille_dots_12456braille_dots_3456braille_dots_13456braille_dots_23456braille_dots_123456braille_dots_7braille_dots_17braille_dots_27braille_dots_127braille_dots_37braille_dots_137braille_dots_237braille_dots_1237braille_dots_47braille_dots_147braille_dots_247braille_dots_1247braille_dots_347braille_dots_1347braille_dots_2347braille_dots_12347braille_dots_57braille_dots_157braille_dots_257braille_dots_1257braille_dots_357braille_dots_1357braille_dots_2357braille_dots_12357braille_dots_457braille_dots_1457braille_dots_2457braille_dots_12457braille_dots_3457braille_dots_13457braille_dots_23457braille_dots_123457braille_dots_67braille_dots_167braille_dots_267braille_dots_1267braille_dots_367braille_dots_1367braille_dots_2367braille_dots_12367braille_dots_467braille_dots_1467braille_dots_2467braille_dots_12467braille_dots_3467braille_dots_13467braille_dots_23467braille_dots_123467braille_dots_567braille_dots_1567braille_dots_2567braille_dots_12567braille_dots_3567braille_dots_13567braille_dots_23567braille_dots_123567braille_dots_4567braille_dots_14567braille_dots_24567braille_dots_124567braille_dots_34567braille_dots_134567braille_dots_234567braille_dots_1234567braille_dots_8braille_dots_18braille_dots_28braille_dots_128braille_dots_38braille_dots_138braille_dots_238braille_dots_1238braille_dots_48braille_dots_148braille_dots_248braille_dots_1248braille_dots_348braille_dots_1348braille_dots_2348braille_dots_12348braille_dots_58braille_dots_158braille_dots_258braille_dots_1258braille_dots_358braille_dots_1358braille_dots_2358braille_dots_12358braille_dots_458braille_dots_1458braille_dots_2458braille_dots_12458braille_dots_3458braille_dots_13458braille_dots_23458braille_dots_123458braille_dots_68braille_dots_168braille_dots_268braille_dots_1268braille_dots_368braille_dots_1368braille_dots_2368braille_dots_12368braille_dots_468braille_dots_1468braille_dots_2468braille_dots_12468braille_dots_3468braille_dots_13468braille_dots_23468braille_dots_123468braille_dots_568braille_dots_1568braille_dots_2568braille_dots_12568braille_dots_3568braille_dots_13568braille_dots_23568braille_dots_123568braille_dots_4568braille_dots_14568braille_dots_24568braille_dots_124568braille_dots_34568braille_dots_134568braille_dots_234568braille_dots_1234568braille_dots_78braille_dots_178braille_dots_278braille_dots_1278braille_dots_378braille_dots_1378braille_dots_2378braille_dots_12378braille_dots_478braille_dots_1478braille_dots_2478braille_dots_12478braille_dots_3478braille_dots_13478braille_dots_23478braille_dots_123478braille_dots_578braille_dots_1578braille_dots_2578braille_dots_12578braille_dots_3578braille_dots_13578braille_dots_23578braille_dots_123578braille_dots_4578braille_dots_14578braille_dots_24578braille_dots_124578braille_dots_34578braille_dots_134578braille_dots_234578braille_dots_1234578braille_dots_678braille_dots_1678braille_dots_2678braille_dots_12678braille_dots_3678braille_dots_13678braille_dots_23678braille_dots_123678braille_dots_4678braille_dots_14678braille_dots_24678braille_dots_124678braille_dots_34678braille_dots_134678braille_dots_234678braille_dots_1234678braille_dots_5678braille_dots_15678braille_dots_25678braille_dots_125678braille_dots_35678braille_dots_135678braille_dots_235678braille_dots_1235678braille_dots_45678braille_dots_145678braille_dots_245678braille_dots_1245678braille_dots_345678braille_dots_1345678braille_dots_2345678braille_dots_12345678hpmute_acutemute_acutehpmute_gravemute_gravehpmute_asciicircummute_asciicircumhpmute_diaeresismute_diaeresishpmute_asciitildemute_asciitildehpliralirahpguilderguilderhpYdiaeresishpIOIOhplongminuslongminushpblockblockDdiaeresisDacute_accentDcedilla_accentDcircumflex_accentDgrave_accentDtildeDring_accentDRemovehpModelock1hpModelock2hpResetResethpSystemSystemhpUserUserhpClearLineClearLinehpInsertLineInsertLinehpDeleteLineDeleteLinehpInsertCharInsertCharhpDeleteCharDeleteCharhpBackTabBackTabhpKP_BackTabKP_BackTabExt16bit_LExt16bit_RosfCopyosfCutosfPasteosfBackTabosfBackSpaceosfClearosfEscapeosfAddModeosfPrimaryPasteosfQuickPasteosfPageLeftosfPageUposfPageDownosfPageRightosfActivateosfMenuBarosfLeftosfUposfRightosfDownosfEndLineosfBeginLineosfEndDataosfBeginDataosfPrevMenuosfNextMenuosfPrevFieldosfNextFieldosfSelectosfInsertosfUndoosfMenuosfCancelosfHelposfSelectAllosfDeselectAllosfReselectosfExtendosfRestoreosfDeleteSunFA_GraveSunFA_CircumSunFA_TildeSunFA_AcuteSunFA_DiaeresisSunFA_CedillaSunF36SunF37SunSys_ReqSunPropsSunFrontSunCopySunOpenSunPasteSunCutSunPowerSwitchSunAudioLowerVolumeSunAudioMuteSunAudioRaiseVolumeSunVideoDegaussSunVideoLowerBrightnessSunVideoRaiseBrightnessSunPowerSwitchShiftXF86MediaPlayPauseXF86ExitXF86AudioBassBoostXF86SportXF86BrightnessAutoXF86MonBrightnessAutoXF86DisplayOffXF86OKXF86GoToXF86InfoXF86VendorLogoXF86MediaSelectProgramGuideXF86MediaSelectHomeXF86MediaLanguageMenuXF86MediaTitleMenuXF86AudioChannelModeXF86AspectRatioXF86MediaSelectPCXF86MediaSelectTVXF86MediaSelectCableXF86MediaSelectVCRXF86MediaSelectVCRPlusXF86MediaSelectSatelliteXF86MediaSelectTapeXF86MediaSelectRadioXF86MediaSelectTunerXF86MediaPlayerXF86MediaSelectTeletextXF86DVDXF86MediaSelectDVDXF86MediaSelectAuxiliaryXF86AudioXF86ChannelUpXF86ChannelDownXF86MediaPlaySlowXF86BreakXF86NumberEntryModeXF86VideoPhoneXF86ZoomResetXF86EditorXF86GraphicsEditorXF86PresentationXF86DatabaseXF86VoicemailXF86AddressbookXF86DisplayToggleXF86SpellCheckXF86ContextMenuXF86MediaRepeatXF8610ChannelsUpXF8610ChannelsDownXF86ImagesXF86NotificationCenterXF86PickupPhoneXF86HangupPhoneXF86LinkPhoneXF86FnXF86Fn_EscXF86Fn_F1XF86Fn_F2XF86Fn_F3XF86Fn_F4XF86Fn_F5XF86Fn_F6XF86Fn_F7XF86Fn_F8XF86Fn_F9XF86Fn_F10XF86Fn_F11XF86Fn_F12XF86Fn_1XF86Fn_2XF86Fn_DXF86Fn_EXF86Fn_FXF86Fn_SXF86Fn_BXF86FnRightShiftXF86Numeric0XF86Numeric1XF86Numeric2XF86Numeric3XF86Numeric4XF86Numeric5XF86Numeric6XF86Numeric7XF86Numeric8XF86Numeric9XF86NumericStarXF86NumericPoundXF86NumericAXF86NumericBXF86NumericCXF86NumericDXF86CameraFocusXF86WPSButtonXF86CameraZoomInXF86CameraZoomOutXF86CameraUpXF86CameraDownXF86CameraLeftXF86CameraRightXF86AttendantOnXF86AttendantOffXF86AttendantToggleXF86LightsToggleXF86ALSToggleXF86RefreshRateToggleXF86ButtonconfigXF86TaskmanagerXF86JournalXF86ControlPanelXF86AppSelectXF86ScreensaverXF86VoiceCommandXF86AssistantXF86EmojiPickerXF86DictateXF86CameraAccessEnableXF86CameraAccessDisableXF86CameraAccessToggleXF86AccessibilityXF86DoNotDisturbXF86BrightnessMinXF86BrightnessMaxXF86ElectronicPrivacyScreenOnXF86ElectronicPrivacyScreenOffXF86ActionOnSelectionXF86ContextualInsertXF86ContextualQueryXF86KbdInputAssistPrevXF86KbdInputAssistNextXF86KbdInputAssistPrevgroupXF86KbdInputAssistNextgroupXF86KbdInputAssistAcceptXF86KbdInputAssistCancelXF86RightUpXF86RightDownXF86LeftUpXF86LeftDownXF86RootMenuXF86MediaTopMenuXF86Numeric11XF86Numeric12XF86AudioDescXF863DModeXF86NextFavoriteXF86StopRecordXF86PauseRecordXF86VODXF86UnmuteXF86FastReverseXF86SlowReverseXF86DataXF86OnScreenKeyboardXF86PrivacyScreenToggleXF86SelectiveScreenshotXF86NextElementXF86PreviousElementXF86AutopilotEngageToggleXF86MarkWaypointXF86SosXF86NavChartXF86FishingChartXF86SingleRangeRadarXF86DualRangeRadarXF86RadarOverlayXF86TraditionalSonarXF86ClearvuSonarXF86SidevuSonarXF86NavInfoXF86Macro1XF86Macro2XF86Macro3XF86Macro4XF86Macro5XF86Macro6XF86Macro7XF86Macro8XF86Macro9XF86Macro10XF86Macro11XF86Macro12XF86Macro13XF86Macro14XF86Macro15XF86Macro16XF86Macro17XF86Macro18XF86Macro19XF86Macro20XF86Macro21XF86Macro22XF86Macro23XF86Macro24XF86Macro25XF86Macro26XF86Macro27XF86Macro28XF86Macro29XF86Macro30XF86MacroRecordStartXF86MacroRecordStopXF86MacroPresetCycleXF86MacroPreset1XF86MacroPreset2XF86MacroPreset3XF86KbdLcdMenu1XF86KbdLcdMenu2XF86KbdLcdMenu3XF86KbdLcdMenu4XF86KbdLcdMenu5XF86PerformanceModeXF86Switch_VT_1XF86Switch_VT_2XF86Switch_VT_3XF86Switch_VT_4XF86Switch_VT_5XF86Switch_VT_6XF86Switch_VT_7XF86Switch_VT_8XF86Switch_VT_9XF86Switch_VT_10XF86Switch_VT_11XF86Switch_VT_12XF86UngrabXF86ClearGrabXF86Next_VModeXF86Prev_VModeXF86LogWindowTreeXF86LogGrabInfoXF86ModeLockXF86MonBrightnessUpXF86MonBrightnessDownXF86KbdLightOnOffXF86KbdBrightnessUpXF86KbdBrightnessDownXF86MonBrightnessCycleXF86StandbyXF86AudioLowerVolumeXF86AudioMuteXF86AudioRaiseVolumeXF86AudioPlayXF86AudioStopXF86AudioPrevXF86AudioNextXF86HomePageXF86MailXF86StartXF86SearchXF86AudioRecordXF86CalculatorXF86MemoXF86ToDoListXF86CalendarXF86PowerDownXF86ContrastAdjustXF86RockerUpXF86RockerDownXF86RockerEnterXF86BackXF86ForwardXF86StopXF86RefreshXF86PowerOffXF86WakeUpXF86EjectXF86ScreenSaverXF86WWWXF86SleepXF86FavoritesXF86AudioPauseXF86AudioMediaXF86MyComputerXF86VendorHomeXF86LightBulbXF86ShopXF86HistoryXF86OpenURLXF86AddFavoriteXF86HotLinksXF86BrightnessAdjustXF86FinanceXF86CommunityXF86AudioRewindXF86BackForwardXF86Launch0XF86Launch1XF86Launch2XF86Launch3XF86Launch4XF86Launch5XF86Launch6XF86Launch7XF86Launch8XF86Launch9XF86LaunchAXF86LaunchBXF86LaunchCXF86LaunchDXF86LaunchEXF86LaunchFXF86ApplicationLeftXF86ApplicationRightXF86BookXF86CDXF86MediaSelectCDXF86CalculaterXF86ClearXF86CloseXF86CopyXF86CutXF86DisplayXF86DOSXF86DocumentsXF86ExcelXF86ExplorerXF86GameXF86GoXF86iTouchXF86LogOffXF86MarketXF86MeetingXF86MenuKBXF86MenuPBXF86MySitesXF86NewXF86NewsXF86OfficeHomeXF86OpenXF86OptionXF86PasteXF86PhoneXF86QXF86ReplyXF86ReloadXF86RotateWindowsXF86RotationPBXF86RotationKBXF86SaveXF86ScrollUpXF86ScrollDownXF86ScrollClickXF86SendXF86SpellXF86SplitScreenXF86SupportXF86TaskPaneXF86TerminalXF86ToolsXF86TravelXF86UserPBXF86User1KBXF86User2KBXF86VideoXF86WheelButtonXF86WordXF86XferXF86ZoomInXF86ZoomOutXF86AwayXF86MessengerXF86WebCamXF86MailForwardXF86PicturesXF86MusicXF86BatteryXF86BluetoothXF86WLANXF86UWBXF86AudioForwardXF86AudioRepeatXF86AudioRandomPlayXF86SubtitleXF86AudioCycleTrackXF86CycleAngleXF86FrameBackXF86FrameForwardXF86TimeXF86SelectXF86ViewXF86TopMenuXF86RedXF86GreenXF86YellowXF86BlueXF86SuspendXF86HibernateXF86TouchpadToggleXF86TouchpadOnXF86TouchpadOffXF86AudioMicMuteXF86KeyboardXF86WWANXF86RFKillXF86AudioPresetXF86RotationLockToggleXF86FullScreen";

pub(super) static KEYSYM_TO_IDX: PhfMap<u32, u16> = {
    let Ok(map) = PhfMap::new(10656210946825422025, &[(1, 2), (1, 461), (1, 6), (0, 0), (0, 105), (0, 21), (0, 262), (0, 1), (1, 24), (0, 2), (1, 44), (0, 164), (0, 6), (0, 520), (0, 26), (0, 101), (0, 30), (0, 276), (0, 0), (0, 571), (0, 136), (0, 11), (0, 9), (0, 6), (1, 2), (0, 1), (0, 40), (1, 13), (0, 15), (0, 28), (0, 258), (0, 11), (0, 27), (0, 471), (0, 196), (0, 64), (0, 363), (0, 289), (0, 5), (0, 1258), (0, 11), (1, 224), (0, 1235), (0, 1037), (0, 2439), (0, 333), (0, 1530), (0, 854), (0, 3), (1, 4), (0, 24), (0, 6), (1, 6), (0, 3), (1, 450), (0, 1865), (0, 3), (0, 216), (0, 56), (0, 2077), (0, 127), (0, 268), (0, 21), (0, 7), (0, 521), (0, 27), (0, 330), (0, 21), (0, 33), (0, 278), (0, 6), (0, 304), (3, 317), (0, 118), (0, 6), (0, 427), (0, 13), (0, 0), (0, 1761), (0, 5), (0, 2), (3, 365), (0, 6), (0, 29), (0, 0), (0, 0), (0, 64), (0, 58), (0, 139), (0, 633), (0, 217), (0, 2), (0, 14), (0, 8), (0, 657), (0, 6), (0, 27), (0, 88), (0, 1), (0, 53), (0, 9), (0, 1), (0, 882), (0, 83), (0, 15), (0, 7), (0, 62), (0, 10), (0, 317), (0, 66), (0, 54), (1, 199), (0, 1182), (0, 2), (0, 11), (0, 1), (0, 32), (0, 21), (0, 2), (0, 28), (0, 12), (0, 0), (0, 2), (0, 2), (0, 7), (0, 29), (0, 0), (0, 50), (0, 20), (0, 14), (0, 86), (0, 98), (0, 1), (0, 2), (1, 17), (1, 0), (0, 1), (1, 1), (0, 30), (0, 2), (0, 37), (0, 3), (0, 386), (0, 1019), (0, 20), (0, 0), (0, 55), (0, 0), (0, 165), (1, 1884), (0, 5), (0, 2321), (0, 150), (0, 6), (0, 1), (0, 5), (0, 8), (1, 1186), (0, 49), (0, 14), (0, 2093), (1, 6), (0, 117), (1, 152), (1, 2), (0, 2), (0, 75), (1, 0), (1, 8), (2, 26), (1, 189), (0, 830), (0, 1536), (0, 1), (1, 0), (1, 257), (0, 22), (0, 178), (1, 20), (1, 8), (0, 59), (0, 24), (1, 293), (0, 648), (0, 212), (0, 706), (0, 149), (0, 8), (0, 741), (0, 841), (0, 2), (0, 1), (0, 24), (0, 477), (1, 0), (0, 1), (0, 320), (0, 40), (1, 8), (0, 822), (0, 1), (0, 2208), (0, 4), (0, 1597), (0, 631), (1, 1121), (0, 5), (1, 632), (0, 377), (0, 5), (3, 1987), (0, 201), (0, 867), (1, 1314), (0, 252), (0, 1100), (0, 63), (0, 117), (0, 1158), (1, 20), (0, 25), (0, 1422), (0, 8), (1, 194), (2, 225), (1, 57), (0, 22), (2, 412), (0, 0), (1, 447), (1, 124), (2, 769), (0, 29), (0, 615), (0, 358), (0, 268), (0, 41), (0, 200), (0, 182), (0, 2), (1, 4), (0, 2), (0, 37), (2, 808), (0, 1421), (0, 24), (2, 2257), (0, 2), (0, 151), (1, 827), (0, 32), (4, 1401), (0, 17), (0, 30), (0, 84), (0, 3), (0, 2465), (0, 0), (0, 22), (0, 1442), (0, 6), (3, 31), (1, 1819), (0, 1), (0, 18), (10, 2142), (0, 19), (0, 10), (4, 1381), (0, 6), (0, 724), (0, 3), (0, 10), (0, 32), (0, 142), (0, 5), (0, 47), (0, 34), (0, 0), (0, 1116), (0, 104), (0, 421), (2, 468), (0, 19), (0, 120), (0, 166), (0, 2), (0, 78), (0, 399), (0, 0), (0, 22), (0, 36), (0, 0), (0, 33), (0, 10), (0, 4), (0, 4), (0, 4), (0, 12), (0, 0), (0, 6), (0, 6), (0, 4), (0, 4), (0, 277), (0, 3), (0, 2), (0, 49), (0, 469), (0, 27), (0, 269), (0, 276), (0, 878), (0, 221), (0, 583), (0, 0), (0, 627), (0, 13), (0, 0), (0, 682), (1, 1200), (0, 24), (2, 2224), (0, 1), (0, 310), (0, 330), (0, 0), (0, 1), (0, 1559), (0, 243), (0, 272), (0, 1712), (1, 51), (0, 1307), (1, 145), (0, 222), (0, 1652), (1, 5), (0, 0), (1, 352), (0, 489), (1, 25), (3, 1602), (0, 16), (1, 0), (3, 425), (0, 32), (2, 14), (0, 222), (0, 11), (1, 2), (1, 694), (0, 43), (1, 1556), (6, 434), (0, 66), (0, 225), (0, 141), (2, 1784), (0, 4), (0, 271), (1, 1734), (0, 1), (0, 1091), (0, 1683), (0, 0), (0, 753), (2, 1687), (0, 0), (4, 660), (1, 237), (0, 2), (0, 431), (1, 877), (0, 632), (4, 820), (0, 116), (12, 2312), (4, 471), (0, 10), (2, 853), (0, 296), (1, 187), (39, 8), (0, 100), (0, 511), (0, 2222), (18, 923), (1, 2), (3, 23), (0, 1), (1, 1), (0, 2131), (0, 1405), (0, 1007), (1, 461), (1, 482), (0, 144), (1, 2161), (0, 398), (5, 33), (0, 1158), (0, 1383), (0, 2274), (1, 148), (7, 2019), (1, 1030), (0, 328), (3, 1424), (0, 385), (1, 17), (0, 54), (7, 577), (0, 18), (22, 77), (32, 281), (1, 1425), (0, 2128), (52, 292), (0, 1225), (0, 8), (0, 477), (11, 1757), (0, 859), (0, 65), (0, 5), (0, 2), (1, 864), (0, 11), (0, 49), (8, 2148), (1, 130), (1, 823), (7, 72), (0, 12), (0, 1025), (0, 195), (0, 0), (0, 447), (0, 195), (0, 13), (0, 134), (0, 1), (2, 778), (0, 888), (0, 21), (0, 0), (3, 830), (1, 2), (0, 195), (0, 440), (40, 2388), (0, 39), (0, 59), (0, 4), (37, 1180), (0, 2), (0, 482), (0, 8), (0, 20), (0, 2), (0, 39), (0, 8), (0, 0), (0, 1), (0, 16), (0, 19), (0, 0), (1, 1475), (1, 800), (0, 61), (0, 302), (0, 513), (0, 4), (0, 451), (1, 3), (0, 1), (0, 50), (6, 612), (0, 73), (0, 0), (1, 6), (0, 12), (0, 25), (2, 1287), (0, 84), (0, 4), (54, 2090), (0, 1), (0, 0), (0, 1167), (0, 953), (4, 1286), (7, 1689), (0, 6), (0, 577), (3, 656), (0, 8), (0, 78), (8, 215), (0, 8), (6, 705), (1, 518), (1, 1), (36, 1646), (0, 18), (1, 7), (2, 1439), (0, 11), (1, 3), (21, 1666), (1, 4)], &[585, 1528, 660, 1724, 2266, 1808, 304, 2042, 201, 1625, 370, 1505, 620, 1802, 727, 77, 1484, 845, 373, 1619, 674, 2031, 1302, 1847, 2237, 2217, 1951, 1014, 782, 1204, 274, 899, 439, 2441, 541, 729, 862, 155, 2022, 1864, 2308, 1319, 1605, 1040, 2157, 926, 2448, 989, 2107, 42, 1054, 2480, 1894, 713, 1144, 892, 1721, 2033, 2479, 2040, 595, 2231, 908, 108, 1338, 1629, 1593, 1688, 708, 861, 210, 388, 754, 1749, 2582, 526, 224, 264, 1092, 647, 267, 778, 983, 2117, 2600, 697, 1943, 2430, 549, 237, 1777, 2407, 29, 2518, 918, 287, 2034, 2210, 2565, 2500, 920, 878, 1188, 104, 2389, 1493, 668, 644, 465, 748, 1267, 1598, 2350, 2509, 90, 1977, 816, 167, 2488, 1632, 406, 1628, 1086, 944, 2282, 2185, 1771, 284, 963, 2285, 235, 939, 652, 888, 1902, 2390, 1670, 344, 1502, 392, 1964, 1342, 588, 1448, 2528, 1995, 633, 209, 579, 1521, 1745, 1364, 288, 508, 2586, 312, 670, 812, 2120, 1648, 99, 2443, 1114, 1511, 428, 2625, 2190, 1237, 421, 1853, 849, 2536, 1839, 799, 1838, 502, 2555, 1295, 2618, 2224, 246, 1651, 1725, 195, 17, 149, 2373, 117, 649, 350, 765, 1729, 1602, 991, 1984, 221, 1170, 8, 2336, 1106, 1734, 707, 547, 773, 22, 947, 2371, 1850, 334, 1063, 1835, 2382, 2013, 2628, 1934, 1249, 976, 1445, 1194, 1681, 205, 653, 391, 198, 1072, 1871, 1795, 2470, 2475, 377, 451, 2090, 512, 1029, 1414, 161, 336, 53, 1623, 1844, 858, 397, 1799, 19, 188, 822, 1252, 1035, 897, 1116, 2203, 1577, 675, 2559, 141, 2622, 1467, 737, 1213, 1141, 530, 1675, 2571, 2150, 1960, 2001, 2461, 601, 440, 1422, 1120, 1529, 1714, 1867, 641, 358, 1940, 1687, 2482, 1032, 2361, 1308, 1523, 162, 592, 1520, 1069, 1895, 2460, 2108, 56, 1138, 1185, 1133, 2473, 2399, 1225, 2100, 2037, 217, 985, 1535, 671, 2297, 700, 2503, 432, 1952, 2064, 1578, 1394, 1889, 2471, 120, 994, 1460, 572, 2324, 536, 927, 1805, 2634, 859, 2241, 2604, 2376, 153, 2388, 1563, 1829, 2454, 2574, 1051, 2362, 2232, 1739, 497, 1215, 2014, 456, 1285, 2426, 83, 1001, 2290, 1978, 371, 2601, 1778, 2118, 1636, 921, 263, 1136, 1912, 1647, 1157, 865, 564, 2213, 2291, 2111, 1453, 2408, 1312, 1179, 1307, 1645, 687, 47, 1435, 1762, 1077, 1057, 1988, 409, 1360, 335, 353, 2027, 1620, 2607, 998, 853, 1957, 2322, 578, 696, 1168, 1131, 2116, 2055, 1352, 292, 2152, 593, 1087, 2580, 1452, 2419, 2552, 78, 1884, 725, 2249, 171, 717, 2029, 615, 1348, 1192, 1982, 2182, 1150, 2028, 363, 1925, 2481, 2061, 757, 91, 1788, 107, 528, 1, 521, 495, 2020, 2077, 2010, 2191, 1238, 686, 1205, 2494, 945, 2437, 635, 788, 883, 2436, 184, 1662, 1557, 2039, 1747, 1186, 704, 951, 2395, 2260, 2345, 208, 2561, 2605, 242, 2286, 556, 2327, 1300, 2384, 72, 2154, 1570, 2342, 1603, 1782, 1712, 1806, 1004, 689, 882, 1476, 643, 575, 433, 2431, 1265, 1743, 200, 724, 618, 2569, 1210, 1065, 1431, 1939, 1020, 96, 1379, 1872, 732, 1316, 313, 1125, 436, 384, 1012, 2570, 1764, 612, 1011, 1064, 135, 977, 1845, 2457, 446, 2502, 1554, 2267, 2421, 1180, 2623, 1800, 1078, 102, 1561, 1949, 1159, 1389, 2513, 1126, 640, 1737, 2306, 1486, 2365, 1117, 2611, 1039, 1473, 2141, 1187, 987, 1633, 1898, 1038, 648, 2204, 1914, 475, 952, 1052, 2485, 1571, 1668, 1412, 51, 402, 127, 1201, 286, 924, 1562, 912, 1074, 1677, 305, 1686, 1304, 231, 1975, 981, 1462, 513, 1540, 1666, 1231, 533, 36, 1585, 2255, 916, 1823, 752, 462, 2452, 2273, 2598, 1775, 412, 1480, 1430, 1046, 1455, 1860, 331, 1404, 826, 228, 2459, 389, 1119, 1286, 1508, 1828, 1694, 206, 2514, 1438, 684, 1969, 1222, 584, 28, 143, 1090, 1903, 1495, 2235, 1083, 1068, 733, 298, 1165, 307, 746, 103, 2143, 1855, 1756, 1118, 46, 233, 1318, 2214, 2176, 342, 225, 114, 787, 21, 2058, 1415, 1769, 2094, 656, 870, 1993, 485, 176, 1446, 2319, 2284, 2251, 1836, 2526, 2401, 1926, 1366, 999, 2579, 1793, 52, 936, 795, 2523, 203, 576, 426, 1323, 2418, 140, 1646, 2616, 81, 2248, 529, 15, 793, 1293, 1663, 1346, 190, 570, 880, 1494, 1753, 2132, 1510, 491, 367, 763, 2553, 2229, 192, 1041, 1596, 1987, 419, 1103, 624, 705, 2610, 152, 2295, 1787, 2367, 1999, 290, 948, 1485, 2375, 1183, 2070, 1313, 1443, 1942, 726, 1879, 974, 1716, 1061, 2468, 871, 1932, 293, 625, 2547, 2032, 2314, 2492, 1025, 1408, 2549, 1900, 1461, 148, 1376, 1679, 2076, 2109, 2025, 33, 1797, 1450, 1112, 2261, 789, 786, 70, 6, 2088, 848, 1958, 771, 404, 2572, 1400, 1746, 2216, 2304, 2592, 2455, 2489, 278, 1281, 2541, 2446, 324, 2495, 1842, 2155, 820, 2620, 621, 1385, 194, 369, 1049, 895, 1869, 1279, 783, 2287, 387, 1854, 182, 2587, 1560, 646, 1211, 1534, 599, 2444, 2564, 1410, 1044, 482, 1558, 39, 310, 1996, 1626, 930, 2103, 1699, 227, 476, 925, 1887, 275, 1703, 101, 2007, 1552, 2177, 1358, 1590, 226, 1624, 507, 1908, 1556, 2505, 959, 488, 2230, 97, 1487, 913, 489, 1007, 1976, 857, 2080, 1735, 2092, 2262, 1406, 1439, 1234, 1824, 1667, 851, 306, 437, 1748, 755, 1093, 179, 2599, 1776, 1634, 1336, 846, 65, 1548, 863, 239, 2065, 2068, 2053, 583, 2130, 1191, 483, 801, 1565, 2539, 1027, 1815, 1465, 841, 850, 2238, 413, 2511, 407, 784, 2483, 1447, 996, 967, 1097, 1573, 730, 1220, 619, 1770, 986, 1311, 129, 1599, 1837, 2110, 1673, 2524, 794, 1713, 780, 319, 889, 2533, 236, 2393, 2315, 2089, 2629, 1553, 1660, 1037, 1732, 1817, 2474, 1301, 520, 181, 2277, 694, 960, 115, 1630, 1148, 1591, 1708, 1862, 1113, 268, 1280, 429, 2377, 797, 553, 2347, 16, 420, 248, 2126, 747, 2134, 506, 1284, 1980, 2323, 1621, 2223, 919, 2603, 1383, 723, 125, 589, 2543, 68, 2379, 809, 1654, 2178, 1601, 2023, 2008, 299, 2047, 2188, 937, 1173, 2147, 2099, 2202, 1517, 2002, 1689, 1791, 1641, 968, 1516, 1876, 2429, 904, 431, 1937, 63, 1780, 543, 1416, 591, 58, 903, 844, 2337, 2069, 1870, 1717, 555, 975, 303, 434, 1425, 121, 2484, 361, 133, 1613, 400, 79, 1328, 94, 1115, 569, 1843, 664, 2072, 2621, 160, 20, 691, 1377, 1076, 2451, 1432, 1254, 1261, 881, 1208, 1674, 969, 673, 1199, 898, 523, 2051, 44, 1896, 1643, 600, 1798, 1417, 1424, 1036, 1378, 1618, 1878, 250, 552, 376, 1344, 2411, 511, 2205, 580, 1142, 609, 616, 2227, 1928, 1963, 906, 1050, 499, 1973, 1761, 1489, 473, 531, 735, 212, 2298, 1330, 2562, 1684, 1877, 2585, 1927, 979, 1583, 2258, 2534, 2567, 2222, 1030, 537, 2596, 230, 1498, 876, 1310, 545, 329, 2030, 459, 1392, 1305, 85, 501, 1356, 1840, 2097, 100, 2635, 50, 2159, 2349, 1321, 1003, 1298, 2593, 1858, 1018, 2391, 394, 2215, 1496, 1592, 2050, 1143, 940, 1913, 701, 220, 1081, 1614, 355, 2173, 240, 1228, 868, 1905, 2104, 1000, 2422, 1166, 1701, 191, 1466, 2212, 1644, 1478, 2383, 186, 1991, 2250, 2220, 645, 1710, 185, 1589, 87, 2012, 1289, 716, 424, 2283, 2123, 1649, 2397, 323, 695, 2105, 1110, 2386, 291, 1924, 561, 1291, 776, 214, 1066, 2148, 2551, 1171, 271, 2416, 417, 2352, 204, 891, 486, 1440, 962, 1399, 931, 1661, 1915, 2487, 1354, 1967, 810, 2428, 1492, 997, 1283, 1611, 1472, 1100, 703, 554, 1635, 2199, 1767, 1243, 71, 761, 2244, 658, 2608, 1059, 2544, 2062, 241, 1917, 972, 524, 2079, 1656, 2063, 1985, 568, 2380, 1883, 2149, 211, 1175, 1060, 2632, 146, 1946, 2128, 1997, 2163, 2310, 173, 105, 1023, 2136, 2195, 60, 657, 678, 1482, 829, 1315, 2225, 2263, 517, 1566, 744, 682, 1676, 941, 1419, 1209, 31, 518, 163, 2374, 1374, 966, 1875, 89, 1722, 174, 2240, 383, 261, 2516, 525, 1956, 2073, 1070, 2056, 1773, 54, 1154, 749, 1821, 2311, 2017, 965, 1706, 2348, 824, 245, 1856, 667, 1260, 607, 1433, 1129, 1950, 1697, 1203, 1736, 2309, 1457, 855, 2081, 1834, 990, 2236, 1390, 474, 1509, 48, 341, 1079, 2403, 382, 2576, 1639, 728, 1428, 37, 1811, 2207, 2289, 0, 1314, 1813, 1454, 1550, 311, 1411, 911, 1970, 2043, 326, 1638, 2228, 277, 1781, 923, 753, 2512, 1541, 815, 984, 2044, 772, 2275, 1091, 2316, 2597, 698, 1479, 463, 1911, 13, 917, 1671, 1584, 2520, 238, 2630, 177, 1774, 1122, 347, 1723, 2264, 1277, 2038, 1421, 2035, 683, 1650, 654, 1968, 2326, 1692, 1738, 1221, 2303, 1189, 151, 745, 2313, 1597, 2259, 1082, 2087, 1617, 2447, 943, 1370, 817, 2537, 877, 2591, 1322, 1818, 887, 2318, 1685, 2531, 1728, 301, 1768, 427, 59, 610, 2498, 914, 1658, 685, 1340, 659, 2472, 2542, 116, 2406, 955, 490, 1931, 2341, 2186, 2573, 357, 14, 480, 88, 1551, 1169, 1111, 715, 623, 2189, 2394, 405, 1158, 2006, 692, 1861, 418, 2558, 1123, 879, 1627, 2211, 339, 1058, 2045, 272, 2525, 2279, 1851, 62, 714, 1986, 1527, 351, 764, 256, 1182, 838, 949, 2067, 2425, 1923, 7, 636, 582, 710, 2200, 1804, 2300, 1849, 721, 720, 2496, 2427, 902, 1547, 2183, 2179, 1176, 833, 973, 617, 2550, 2254, 1420, 401, 1866, 2048, 1705, 189, 807, 1153, 1303, 454, 2000, 1594, 842, 57, 1609, 380, 606, 1868, 213, 1449, 1796, 1682, 821, 832, 1841, 1622, 1251, 2435, 1105, 86, 1532, 896, 2196, 2560, 2627, 535, 223, 1214, 1543, 253, 1542, 676, 1863, 1246, 2449, 1794, 2328, 469, 843, 740, 1034, 18, 598, 1587, 300, 2508, 1733, 158, 1857, 262, 2385, 1048, 680, 1672, 1140, 2463, 2636, 55, 805, 2024, 1101, 1961, 1755, 1005, 2018, 539, 993, 634, 112, 1955, 1759, 1423, 2434, 1537, 82, 1581, 1393, 928, 457, 137, 2106, 259, 1888, 2364, 2340, 2151, 1226, 1698, 1572, 398, 594, 860, 1042, 769, 2175, 1028, 327, 25, 551, 2583, 229, 630, 669, 1574, 2208, 1826, 2086, 874, 1965, 297, 1816, 1665, 1177, 1132, 672, 758, 1518, 1885, 2396, 2095, 2355, 922, 2507, 1812, 2415, 1096, 866, 719, 1765, 2589, 2169, 565, 2540, 934, 2510, 395, 1506, 605, 1642, 1947, 1576, 574, 1145, 1989, 2501, 995, 1332, 2145, 1935, 2184, 2420, 702, 1763, 1426, 1702, 1906, 767, 611, 109, 172, 2614, 345, 11, 1920, 901, 1401, 415, 2504, 2218, 854, 1306, 2115, 2288, 2335, 2325, 269, 662, 1163, 496, 1350, 566, 2247, 106, 386, 1149, 1919, 596, 1524, 2469, 1719, 362, 813, 559, 1709, 628, 2358, 2445, 2606, 1098, 935, 4, 2497, 2532, 2046, 1659, 1241, 1859, 886, 1983, 199, 2633, 2257, 2112, 1930, 273, 41, 1108, 2440, 1491, 1945, 1655, 1444, 320, 2442, 2466, 2378, 1441, 1893, 2084, 1606, 1458, 356, 123, 92, 1135, 2577, 1848, 2021, 1929, 1362, 95, 1683, 2522, 954, 1402, 2271, 2281, 296, 1075, 1742, 26, 2568, 435, 2293, 2049, 1569, 168, 516, 309, 718, 1282, 2491, 2476, 1616, 1490, 1207, 134, 2193, 1944, 1396, 1938, 2400, 597, 23, 138, 1010, 113, 1803, 1015, 399, 1468, 1043, 1704, 2015, 183, 187, 279, 2363, 915, 1948, 35, 1481, 330, 988, 1907, 1500, 2499, 1200, 837, 1256, 1545, 2209, 964, 665, 603, 1881, 1899, 2414, 378, 178, 1294, 2201, 562, 540, 1055, 1471, 2206, 232, 2398, 375, 364, 1326, 938, 586, 2626, 471, 504, 2226, 1972, 147, 2356, 992, 218, 1085, 1276, 1539, 276, 642, 909, 1700, 1901, 2519, 751, 534, 2083, 1772, 2272, 266, 847, 2490, 1089, 790, 1229, 1474, 403, 875, 1519, 1477, 661, 2357, 2019, 791, 460, 546, 2404, 154, 681, 1437, 1021, 2005, 2381, 1827, 1966, 1751, 1127, 1219, 1730, 2372, 2098, 1156, 1266, 2578, 69, 1218, 1994, 1080, 639, 2359, 2331, 1909, 839, 2517, 234, 743, 1056, 1595, 283, 343, 1726, 40, 869, 781, 2122, 608, 425, 1582, 1760, 509, 1715, 1368, 75, 165, 1002, 1388, 1512, 1874, 1890, 2059, 2339, 448, 157, 1567, 905, 942, 2004, 2321, 12, 712, 1073, 560, 2307, 542, 2615, 2590, 690, 2529, 505, 2082, 1744, 1631, 315, 487, 1196, 1652, 1555, 2368, 196, 416, 2486, 34, 74, 1381, 295, 368, 970, 144, 308, 2464, 1992, 118, 1610, 1822, 1669, 5, 2198, 2556, 1786, 2221, 1242, 1407, 2609, 828, 779, 2085, 900, 1921, 243, 2234, 768, 1174, 1195, 651, 971, 2294, 1372, 1754, 840, 1531, 741, 1504, 280, 1882, 1640, 2093, 1998, 2424, 933, 1022, 1409, 650, 32, 202, 1941, 1047, 317, 2194, 132, 604, 852, 452, 2187, 2477, 2041, 819, 349, 2242, 2256, 1959, 932, 1538, 1678, 1212, 2535, 1810, 410, 1549, 1766, 119, 736, 2346, 1918, 396, 1892, 372, 738, 98, 2548, 2392, 1612, 894, 1429, 622, 1016, 2637, 270, 1820, 1522, 302, 2239, 2161, 442, 1397, 337, 637, 251, 1217, 1757, 1696, 2268, 731, 1121, 1695, 1536, 1384, 38, 1953, 1258, 1886, 2478, 477, 2387, 1206, 762, 1637, 338, 2276, 1830, 1320, 1151, 632, 1172, 139, 2036, 43, 2075, 374, 340, 2631, 1615, 2233, 1691, 910, 1008, 252, 1568, 1814, 2243, 1690, 2453, 2575, 756, 872, 2506, 1216, 1033, 2329, 2138, 325, 180, 1094, 856, 2602, 1779, 699, 494, 614, 563, 2119, 1783, 1792, 1910, 1130, 1990, 318, 2052, 2278, 408, 864, 1299, 1451, 222, 1167, 1680, 785, 1190, 1317, 2091, 688, 1971, 957, 2302, 1600, 2296, 24, 2538, 444, 1530, 814, 1463, 1526, 64, 946, 1579, 1488, 2301, 2343, 1979, 2360, 484, 2101, 430, 2351, 2530, 260, 2252, 613, 1161, 170, 958, 1657, 2181, 385, 581, 750, 2011, 2113, 1442, 777, 693, 1147, 2332, 2458, 2171, 510, 2450, 411, 557, 834, 1707, 479, 346, 2192, 197, 2078, 254, 2009, 289, 124, 1088, 360, 1239, 2438, 2557, 1513, 2409, 884, 1026, 722, 130, 1459, 2312, 1727, 1386, 1789, 2584, 893, 1178, 1071, 247, 1809, 519, 93, 627, 1109, 142, 1653, 2066, 1731, 1124, 1604, 709, 1807, 711, 393, 390, 2405, 27, 1607, 759, 2353, 590, 1164, 1740, 2253, 1936, 2071, 2566, 1981, 1418, 1181, 775, 802, 1380, 2370, 1873, 631, 1263, 811, 1507, 136, 2412, 1852, 1013, 2493, 379, 677, 76, 1006, 1833, 1436, 2624, 1846, 2197, 663, 514, 527, 2467, 481, 249, 982, 2417, 1865, 956, 1693, 1469, 49, 1255, 2338, 2563, 2305, 1962, 1752, 1801, 2410, 739, 538, 1586, 258, 467, 1720, 2153, 1515, 348, 73, 978, 1197, 835, 2270, 1053, 550, 1922, 2320, 2146, 1375, 2003, 2333, 2462, 61, 629, 2102, 1525, 1019, 907, 1434, 2581, 1954, 1427, 2299, 2124, 1758, 2369, 1334, 734, 2344, 1891, 1278, 265, 1580, 532, 1514, 1031, 1024, 381, 458, 1309, 770, 215, 2402, 873, 2521, 1413, 1160, 980, 216, 2456, 1819, 1456, 929, 2317, 285, 1825, 836, 314, 257, 1146, 1017, 1405, 544, 1904, 679, 1045, 2330, 1184, 950, 2515, 1382, 453, 1062, 1009, 655, 1155, 175, 2, 2265, 2167, 2594, 1785, 1916, 587, 571, 1095, 1711, 830, 493, 2219, 1974, 867, 1831, 2619, 1403, 282, 111, 2165, 638, 626, 766, 321, 423, 1588, 80, 573, 2074, 1227, 2423, 1475, 742, 2060, 2527, 2613, 2114, 1564, 2121, 2595, 1741, 84, 558, 414, 1790, 1107, 2246, 1470, 10, 1162, 2554, 294, 2588, 1664, 2354, 961, 1297, 2366, 1391, 2612, 1897, 492, 366, 503, 332, 1608, 760, 2280, 2334, 2274, 1250, 164, 2413, 2617, 1099, 169, 2026, 1296, 706, 207, 1240, 2292, 219, 3, 666, 316, 885, 2180, 1483, 2057, 1933, 1152, 1718, 1784, 953, 2465, 567, 131, 1193, 803, 1464, 1750, 1832, 1398, 890, 2545, 2439, 498, 774, 145, 522, 792, 122, 1324, 45, 2432, 2016, 1880, 1067, 500, 66, 1287, 2433, 1559, 255, 30, 359, 1128, 515, 1134, 1288, 328, 244, 156, 1387, 2054, 1533, 2096, 1395, 450]);

    map
};

pub(super) static NAME_TO_IDX: PhfMap<[u8], u16> = {
    let Ok(map) = PhfMap::new(10656210946825422025, &[(0, 15), (0, 0), (0, 0), (0, 16), (0, 1), (0, 1), (0, 4), (0, 0), (0, 7), (0, 0), (0, 1), (0, 0), (0, 7), (0, 4), (0, 0), (0, 41), (0, 0), (0, 54), (0, 4), (0, 99), (0, 6), (0, 18), (0, 14), (0, 10), (0, 2), (0, 0), (0, 3), (0, 42), (0, 0), (0, 7), (0, 0), (0, 47), (0, 3), (0, 14), (0, 83), (0, 3), (0, 60), (0, 65), (0, 2), (0, 2), (0, 4), (0, 4), (0, 2), (0, 1), (0, 10), (0, 0), (0, 3), (0, 56), (0, 0), (0, 4), (0, 0), (0, 2), (0, 0), (0, 0), (0, 0), (0, 5), (0, 11), (0, 0), (0, 4), (0, 32), (0, 3), (0, 2), (0, 1), (0, 0), (0, 5), (0, 9), (0, 0), (0, 4), (0, 2), (0, 1), (0, 2), (0, 39), (0, 0), (0, 3), (0, 6), (0, 0), (0, 9), (0, 6), (0, 0), (0, 19), (0, 23), (0, 2), (0, 15), (0, 11), (0, 16), (0, 0), (0, 30), (0, 3), (0, 0), (0, 0), (0, 57), (0, 2), (0, 9), (0, 11), (0, 1), (0, 1), (0, 2), (0, 26), (0, 2), (0, 3), (0, 13), (0, 3), (0, 0), (0, 26), (0, 0), (0, 0), (0, 0), (0, 0), (0, 1), (0, 4), (0, 10), (0, 12), (0, 105), (0, 3), (0, 2), (0, 7), (0, 1), (0, 0), (0, 2), (0, 0), (0, 15), (0, 25), (0, 0), (0, 0), (0, 5), (0, 19), (0, 0), (0, 33), (0, 9), (0, 5), (0, 4), (0, 7), (0, 8), (0, 3), (0, 0), (0, 36), (0, 1), (0, 2), (0, 10), (0, 65), (0, 12), (0, 18), (0, 1), (0, 5), (0, 28), (0, 0), (0, 0), (0, 70), (0, 18), (0, 29), (0, 35), (0, 2), (0, 0), (0, 16), (0, 2), (0, 0), (0, 36), (0, 1), (0, 10), (0, 4), (0, 26), (0, 3), (0, 6), (0, 4), (0, 31), (0, 1), (0, 64), (0, 345), (0, 65), (0, 1), (0, 1), (0, 65), (0, 0), (0, 14), (0, 22), (0, 6), (0, 12), (0, 8), (0, 0), (0, 46), (0, 43), (0, 0), (0, 80), (0, 55), (0, 530), (0, 1), (0, 14), (0, 12), (0, 5), (0, 5), (0, 0), (0, 39), (0, 3), (0, 10), (0, 15), (0, 18), (0, 6), (0, 0), (0, 6), (0, 0), (0, 18), (0, 0), (0, 3), (0, 14), (0, 50), (0, 0), (0, 8), (0, 36), (0, 0), (0, 2), (0, 0), (0, 60), (0, 28), (0, 30), (0, 5), (0, 23), (0, 11), (0, 0), (0, 11), (0, 5), (0, 3), (0, 30), (0, 423), (0, 41), (0, 0), (0, 146), (0, 2), (0, 81), (0, 3), (0, 1), (0, 0), (0, 9), (0, 13), (0, 27), (0, 0), (0, 3), (0, 118), (0, 17), (0, 22), (0, 2), (0, 2), (0, 99), (0, 2), (0, 102), (0, 18), (0, 0), (0, 19), (0, 37), (0, 0), (0, 11), (0, 127), (0, 1), (0, 7), (0, 36), (0, 14), (0, 29), (0, 2), (0, 79), (0, 5), (0, 0), (0, 0), (0, 23), (0, 9), (0, 0), (0, 0), (0, 6), (0, 0), (0, 50), (0, 0), (0, 0), (0, 23), (0, 0), (0, 7), (0, 1), (0, 1), (0, 80), (0, 8), (0, 19), (0, 12), (0, 1), (0, 1), (0, 6), (0, 0), (0, 10), (0, 2), (0, 45), (0, 2), (0, 21), (0, 1), (0, 2), (0, 1), (0, 9), (0, 33), (0, 0), (0, 8), (0, 48), (0, 74), (0, 0), (0, 0), (0, 282), (0, 301), (0, 0), (0, 0), (0, 5), (0, 4), (0, 52), (0, 23), (0, 0), (0, 19), (0, 8), (0, 7), (0, 7), (0, 8), (0, 5), (0, 0), (0, 0), (0, 0), (0, 415), (0, 0), (0, 19), (0, 0), (0, 51), (0, 2), (0, 2), (0, 13), (0, 1), (0, 86), (0, 0), (0, 14), (0, 19), (0, 52), (0, 0), (0, 4), (0, 13), (0, 4), (0, 92), (0, 0), (0, 14), (0, 13), (0, 0), (0, 45), (0, 0), (0, 19), (0, 14), (0, 5), (0, 5), (0, 1), (0, 1), (0, 36), (0, 15), (0, 0), (0, 72), (0, 2), (0, 7), (0, 21), (0, 26), (0, 0), (0, 4), (0, 0), (0, 20), (0, 6), (0, 31), (0, 0), (0, 0), (0, 0), (0, 40), (0, 68), (0, 0), (0, 0), (0, 2), (0, 0), (0, 42), (0, 1), (0, 4), (0, 4), (0, 0), (0, 3), (0, 0), (0, 86), (0, 175), (0, 156), (0, 15), (0, 9), (0, 293), (0, 40), (0, 627), (0, 0), (0, 14), (0, 2), (0, 0), (0, 2), (0, 0), (0, 17), (0, 0), (0, 4), (0, 8), (0, 3), (0, 10), (0, 22), (0, 0), (0, 14), (0, 12), (0, 6), (0, 4), (0, 15), (0, 0), (0, 38), (0, 25), (0, 11), (0, 12), (0, 1), (0, 2), (0, 178), (0, 5), (0, 9), (0, 7), (0, 2), (0, 124), (0, 339), (0, 3), (0, 23), (0, 149), (0, 0), (0, 3), (0, 4), (0, 0), (0, 6), (0, 0), (0, 30), (0, 31), (0, 0), (0, 1), (0, 19), (0, 0), (0, 1), (0, 563), (0, 7), (0, 0), (0, 4), (0, 110), (0, 14), (0, 2), (0, 30), (0, 2), (0, 7), (0, 0), (0, 0), (0, 14), (0, 7), (0, 16), (0, 57), (0, 8), (0, 6), (0, 4), (0, 2), (0, 121), (0, 0), (0, 2), (0, 454), (0, 0), (0, 4), (0, 392), (0, 7), (0, 34), (0, 240), (0, 4), (0, 2), (0, 22), (0, 9), (0, 620), (0, 0), (0, 40), (0, 431), (0, 271), (0, 32), (0, 28), (0, 11), (0, 14), (0, 5), (0, 28), (0, 70), (0, 126), (0, 73), (0, 38), (0, 52), (0, 6), (0, 1), (0, 133), (0, 128), (0, 27), (0, 18), (0, 0), (0, 31), (0, 1), (0, 38), (0, 23), (0, 72), (0, 3), (0, 89), (0, 26), (0, 11), (0, 137), (0, 1), (0, 10), (0, 0), (0, 0), (0, 2), (0, 5), (0, 8), (0, 1), (0, 32), (0, 2), (0, 0), (0, 51), (0, 46), (0, 0), (0, 3), (0, 0), (0, 5), (0, 62), (0, 26), (0, 6), (0, 25), (0, 0), (0, 0), (0, 42), (0, 19), (0, 16), (0, 3), (0, 0), (0, 5), (0, 4), (0, 47), (0, 0), (0, 0), (0, 2), (0, 7), (0, 37), (0, 4), (0, 0), (0, 4), (0, 7), (0, 56), (0, 4), (0, 0), (0, 220), (0, 23), (0, 1), (0, 70), (0, 0), (0, 2), (0, 5), (0, 0), (0, 120), (0, 7), (0, 0), (0, 0), (0, 154), (0, 1), (0, 2), (0, 0), (0, 0), (0, 119), (0, 334), (0, 3), (0, 0), (0, 5), (0, 16), (0, 16), (0, 0), (0, 1), (0, 0), (0, 2), (0, 147), (0, 7), (0, 0), (0, 77), (0, 16), (0, 13), (0, 144), (0, 2), (0, 37), (0, 33), (0, 1), (0, 125), (0, 12), (0, 55), (0, 93), (0, 77), (0, 47), (0, 0), (0, 2), (0, 727), (0, 330), (0, 22), (0, 0), (0, 33), (0, 95), (0, 21), (0, 20), (0, 43), (0, 336), (0, 53), (0, 0), (0, 10), (0, 2), (0, 264), (0, 529), (0, 0), (0, 43), (0, 1), (0, 30), (0, 180), (0, 6), (0, 155), (0, 58), (0, 20), (0, 109), (0, 2), (0, 697), (0, 5), (0, 8), (0, 1), (0, 0), (0, 40), (0, 49), (0, 57), (0, 59), (0, 1), (0, 0), (0, 5), (0, 5), (0, 0), (0, 0), (0, 212), (0, 103), (0, 5), (0, 5), (0, 120), (0, 6), (0, 2), (0, 18), (0, 7), (0, 112), (0, 18), (0, 0), (0, 135), (0, 12), (0, 5), (0, 21), (0, 1), (0, 820), (0, 0), (0, 19), (0, 7), (0, 51), (0, 354), (0, 0), (0, 139), (0, 216), (0, 566), (0, 37), (0, 0), (0, 40), (0, 48), (0, 536), (0, 64), (0, 1), (0, 222), (0, 1), (0, 182), (0, 1), (0, 10), (0, 28), (0, 14), (0, 0), (0, 93), (0, 0), (0, 626), (0, 1000), (0, 469), (0, 1), (0, 111), (0, 977), (0, 6), (0, 0), (0, 87), (0, 218), (0, 0), (0, 3), (0, 29), (0, 32), (0, 78), (0, 0), (0, 0), (0, 51), (0, 83), (0, 0), (0, 17), (0, 6), (0, 2), (0, 2), (0, 8), (0, 39), (0, 259), (0, 3), (0, 841), (0, 43), (0, 0), (0, 25), (0, 167), (0, 7), (0, 160), (0, 1), (0, 15), (0, 41), (0, 31), (0, 4), (0, 14), (0, 208), (0, 7), (0, 4), (0, 259), (0, 216), (0, 24), (0, 0), (0, 1), (0, 1), (0, 31), (0, 14), (0, 4), (0, 0), (0, 13), (0, 2), (0, 0), (0, 1), (0, 18), (0, 0), (0, 2), (0, 530), (0, 3), (0, 48), (0, 88), (0, 13), (0, 0), (0, 192), (0, 0), (0, 102), (0, 2), (0, 1), (0, 33), (0, 6), (0, 634), (0, 100), (0, 0), (0, 9), (0, 217), (0, 0), (0, 14), (0, 894), (0, 229), (0, 3), (0, 666), (0, 160), (0, 0), (0, 0), (0, 116), (0, 947), (0, 0), (0, 12), (0, 12), (0, 12), (0, 5), (0, 3), (0, 22), (0, 2), (0, 8), (0, 0), (0, 19), (0, 24), (0, 105), (0, 230), (0, 32), (0, 713), (0, 688), (0, 0), (0, 600), (0, 6), (0, 26), (0, 12), (0, 12), (0, 11), (0, 13), (0, 8), (0, 112), (0, 13), (0, 25), (0, 0), (0, 197), (0, 0), (0, 39), (0, 81), (0, 7), (0, 1), (0, 6), (0, 0), (0, 10), (0, 285), (0, 42), (0, 20), (0, 217), (0, 732), (0, 16), (0, 1031), (0, 26), (0, 54), (0, 29), (0, 19), (0, 0), (0, 3), (0, 3), (0, 2), (0, 212), (0, 3), (0, 980), (0, 0), (0, 3), (0, 593), (0, 7), (0, 0), (0, 150), (0, 105), (0, 150), (0, 967), (0, 1027), (0, 154), (0, 7), (0, 3), (0, 454), (0, 211), (0, 4), (0, 12), (0, 1160), (0, 0), (0, 128), (0, 42), (0, 131), (0, 139), (0, 48), (0, 70), (0, 94), (0, 422), (0, 775), (0, 21), (0, 95), (0, 0), (0, 497), (0, 216), (0, 808), (0, 0), (0, 2), (0, 3), (0, 301), (0, 2334), (0, 4), (0, 335), (0, 1), (0, 40), (0, 61), (0, 10), (0, 0), (0, 3), (0, 3), (0, 57), (0, 267), (0, 0), (0, 1), (0, 484), (0, 2221), (0, 0), (0, 1), (0, 38), (0, 2), (0, 1), (0, 2309), (0, 8), (0, 4), (0, 23), (0, 1), (0, 0), (0, 7), (0, 130), (0, 411), (0, 6), (0, 248), (0, 849), (0, 1273), (0, 56), (0, 1626), (0, 11), (0, 0), (0, 45), (0, 560), (0, 1968), (0, 2493), (0, 846), (0, 347), (0, 40), (0, 0), (0, 362), (0, 19), (0, 4), (0, 104), (0, 428), (0, 26), (0, 165), (0, 439), (0, 13), (0, 2229), (0, 0), (0, 21), (0, 1728), (0, 19), (0, 2343), (0, 132), (0, 456), (0, 8), (0, 22), (0, 628), (0, 174), (0, 40), (0, 89), (0, 145), (0, 11), (0, 11), (0, 0), (0, 0), (0, 94), (0, 0), (0, 0), (0, 14), (0, 1644), (0, 899), (0, 46), (0, 204), (0, 8), (0, 0), (0, 0), (0, 1333), (0, 7), (0, 1745), (0, 0), (0, 380), (0, 0), (0, 4), (0, 673), (0, 72), (0, 9), (0, 1319), (0, 0), (0, 37), (0, 126), (0, 0), (0, 1), (0, 120), (0, 0), (0, 4), (0, 25), (0, 35), (0, 38), (0, 1), (0, 1), (0, 44), (0, 3), (0, 149), (0, 5), (0, 202), (0, 43), (0, 36), (0, 1957), (0, 647), (0, 0), (0, 1253), (0, 3), (0, 20), (0, 64), (0, 69), (0, 27), (0, 8), (0, 35), (0, 883), (0, 10), (0, 52), (0, 4), (0, 172), (0, 3), (0, 2), (0, 2), (0, 0), (0, 19), (0, 393), (0, 5), (0, 2), (0, 193), (0, 1), (0, 9), (0, 12), (0, 4), (0, 107), (0, 0), (0, 25), (0, 27), (0, 186), (0, 76), (0, 1317), (0, 336), (0, 74), (0, 355), (0, 676), (0, 1862), (0, 1020), (0, 82), (0, 39), (0, 38)], &[529, 2133, 1191, 420, 255, 654, 2074, 807, 618, 774, 182, 1425, 2420, 171, 1832, 660, 2089, 36, 2267, 1935, 370, 1169, 2500, 563, 890, 1303, 1751, 1574, 132, 1000, 692, 1350, 1658, 1667, 1716, 262, 2146, 2616, 1929, 1452, 1153, 729, 167, 349, 1533, 1332, 557, 2624, 131, 355, 1243, 475, 51, 2439, 1736, 385, 2475, 1370, 502, 282, 1732, 2551, 579, 473, 1376, 1188, 96, 665, 2001, 681, 2544, 2402, 596, 2287, 1649, 451, 960, 2471, 1958, 1294, 1570, 2491, 2518, 2554, 1061, 130, 1408, 79, 1333, 1798, 1564, 2210, 1569, 2409, 2282, 1680, 1409, 917, 98, 545, 1541, 1468, 367, 1321, 1580, 1500, 1867, 2313, 2066, 2547, 1893, 1760, 966, 1804, 333, 1724, 1609, 1365, 2466, 2113, 1357, 2557, 721, 1755, 1097, 1542, 2106, 921, 339, 2534, 316, 2479, 2374, 1017, 1290, 2097, 1780, 2459, 254, 1546, 1484, 2196, 346, 1393, 2555, 2005, 739, 2117, 2450, 1841, 819, 2329, 2412, 2105, 687, 2583, 2598, 1448, 2426, 26, 2224, 2574, 1464, 89, 2318, 797, 1765, 1843, 861, 2273, 940, 427, 1059, 2550, 2243, 1994, 1834, 436, 293, 2317, 1075, 605, 1502, 2499, 29, 2138, 207, 2505, 583, 1727, 2228, 1339, 1688, 25, 2441, 1020, 43, 922, 337, 2256, 2361, 2460, 598, 156, 395, 2501, 2068, 2516, 2058, 1623, 994, 1931, 415, 275, 2132, 2004, 406, 2445, 871, 1278, 2463, 1419, 733, 1495, 1305, 648, 2354, 1337, 426, 1768, 2102, 38, 695, 2634, 2219, 1720, 1887, 2298, 205, 2407, 1792, 1507, 390, 878, 995, 300, 460, 1015, 924, 2073, 2055, 230, 2399, 248, 2362, 1167, 621, 977, 986, 471, 1282, 2172, 364, 1257, 1161, 1629, 53, 161, 261, 1526, 1106, 412, 712, 1905, 962, 2101, 322, 1498, 175, 971, 2521, 2029, 1113, 2170, 1605, 151, 531, 1066, 2151, 844, 1763, 214, 139, 1517, 2617, 1519, 745, 1447, 887, 449, 2347, 1709, 640, 292, 2213, 2588, 81, 2627, 2366, 1417, 129, 1742, 954, 1353, 1380, 857, 1806, 2125, 2568, 485, 1690, 2120, 1396, 872, 1941, 154, 783, 399, 1288, 674, 28, 565, 805, 495, 222, 440, 829, 2174, 2082, 1039, 1286, 914, 1741, 2391, 659, 403, 1949, 134, 187, 2052, 45, 1462, 2166, 680, 603, 1264, 137, 1, 469, 1254, 2489, 1620, 2307, 1067, 2581, 1429, 2424, 772, 2567, 2173, 217, 2116, 2388, 1481, 1923, 810, 1613, 2160, 302, 2549, 1920, 613, 1157, 1092, 1769, 2109, 1748, 1024, 898, 1444, 467, 1921, 1677, 1666, 1022, 1131, 1892, 2300, 1651, 957, 1738, 763, 500, 506, 820, 1885, 778, 2114, 2350, 556, 667, 2290, 2137, 974, 862, 1132, 956, 6, 2345, 351, 2183, 1359, 2249, 162, 1340, 688, 1646, 513, 1485, 860, 690, 1098, 2028, 1611, 2169, 885, 1038, 1997, 1307, 127, 2386, 1784, 1103, 1524, 509, 40, 1753, 332, 1142, 2486, 108, 2293, 991, 158, 756, 99, 2523, 1823, 1661, 867, 1797, 2047, 543, 628, 1571, 1954, 393, 1776, 816, 1656, 195, 514, 1215, 1707, 1342, 852, 814, 1208, 1280, 1461, 1535, 1052, 2083, 1782, 909, 350, 1478, 629, 2435, 1979, 1913, 1637, 1551, 258, 595, 678, 1995, 1881, 1027, 1991, 1785, 2280, 2363, 755, 845, 2134, 620, 537, 201, 1120, 1094, 365, 584, 1387, 948, 157, 1220, 567, 1759, 2587, 198, 1492, 2158, 1928, 1301, 724, 1354, 1349, 55, 1326, 1835, 1265, 1683, 2383, 2222, 569, 1734, 1391, 1833, 2153, 2037, 901, 1701, 1228, 1728, 368, 2582, 359, 576, 709, 1238, 1053, 1946, 121, 581, 627, 1137, 14, 927, 95, 1674, 388, 1042, 1182, 47, 329, 174, 561, 1151, 2480, 1912, 1174, 2197, 1869, 730, 2305, 1673, 2601, 2442, 2371, 883, 1597, 2357, 841, 422, 120, 2506, 952, 808, 700, 2311, 2625, 2536, 2214, 2309, 1003, 720, 2593, 2043, 2078, 2064, 1005, 1689, 2145, 2046, 535, 850, 786, 711, 1940, 1230, 2328, 378, 1641, 751, 1726, 2390, 3, 1573, 1664, 809, 5, 935, 2302, 1148, 1116, 2488, 1160, 548, 1081, 2517, 1685, 2233, 1722, 2258, 246, 1774, 1583, 1933, 429, 1196, 540, 1296, 348, 2342, 481, 1112, 361, 2496, 1445, 961, 570, 439, 2563, 1048, 559, 992, 1040, 1523, 1063, 1171, 2527, 492, 1996, 1071, 1458, 2333, 58, 2538, 1978, 204, 2389, 591, 1465, 102, 2025, 31, 1204, 1346, 1586, 1250, 1245, 1459, 2096, 2033, 2206, 718, 1866, 356, 858, 1762, 1699, 2493, 2111, 2623, 2437, 875, 57, 1626, 110, 2198, 1309, 2156, 1807, 1548, 1553, 1540, 2276, 633, 4, 916, 266, 1272, 1252, 2107, 2369, 748, 1872, 397, 185, 2419, 1126, 1091, 1983, 1691, 708, 1771, 2176, 2060, 780, 69, 443, 1849, 652, 1632, 947, 1302, 1315, 483, 2349, 446, 2414, 1323, 243, 2221, 1520, 1109, 1072, 1183, 1345, 288, 1410, 1119, 1950, 1172, 1910, 232, 2194, 1829, 2212, 2189, 968, 2340, 1655, 49, 1135, 526, 2621, 1700, 638, 236, 760, 1813, 84, 336, 1037, 768, 159, 2220, 900, 505, 2448, 2010, 1986, 2192, 1176, 2093, 497, 496, 1781, 792, 2094, 283, 1073, 2511, 1640, 1438, 1382, 2119, 1360, 503, 716, 944, 1145, 1621, 1825, 2428, 2150, 2564, 226, 1421, 790, 2572, 1263, 752, 1014, 753, 1118, 761, 1479, 2562, 343, 1890, 1402, 769, 1456, 726, 918, 34, 1547, 2296, 311, 2467, 2355, 2279, 1451, 943, 2304, 1029, 796, 1982, 2205, 1442, 328, 510, 317, 1205, 1968, 897, 666, 1206, 1585, 1102, 2131, 518, 113, 2203, 1916, 425, 880, 1508, 744, 604, 1090, 1400, 1270, 1311, 953, 1821, 1714, 1638, 231, 1855, 60, 1868, 2596, 1436, 247, 2331, 1703, 936, 2242, 318, 2091, 1558, 1432, 234, 1034, 2050, 623, 1604, 1503, 2529, 1545, 2591, 1070, 1026, 269, 67, 1622, 1902, 1083, 1670, 1446, 2248, 2338, 1466, 1715, 564, 1531, 1883, 2080, 873, 2325, 76, 1506, 438, 707, 326, 2187, 2181, 767, 912, 1555, 289, 1567, 779, 1594, 1884, 1218, 1671, 574, 148, 1631, 2286, 863, 1970, 913, 1197, 658, 1487, 848, 2600, 2178, 1969, 2070, 1371, 71, 899, 106, 1334, 2580, 334, 1076, 256, 1645, 1665, 946, 2000, 915, 1266, 2560, 1864, 2408, 1390, 1708, 138, 206, 430, 2208, 62, 371, 358, 2180, 2186, 1232, 706, 2540, 86, 2353, 35, 308, 1202, 1659, 416, 421, 1616, 2507, 1733, 1159, 2606, 2619, 2081, 2195, 1100, 383, 2218, 101, 534, 104, 2062, 1249, 218, 1862, 1624, 717, 30, 1775, 2099, 215, 1486, 544, 622, 1489, 441, 1799, 170, 1273, 1744, 2401, 859, 2016, 1737, 2136, 2326, 18, 533, 1966, 2281, 2193, 764, 866, 2009, 2121, 1236, 1412, 1710, 2288, 1198, 463, 1049, 2443, 2546, 1687, 1873, 2358, 1787, 2268, 520, 2246, 345, 1130, 1420, 2100, 886, 486, 122, 2433, 1529, 1009, 1262, 2520, 1471, 1577, 133, 1164, 1579, 601, 523, 1364, 1698, 354, 1214, 1904, 1457, 1300, 270, 1878, 1016, 1630, 136, 1068, 1568, 1521, 405, 704, 1974, 1054, 1915, 868, 1104, 2519, 413, 2274, 225, 1241, 590, 2454, 417, 487, 617, 2124, 2495, 697, 2570, 2122, 1253, 945, 955, 387, 376, 1443, 1697, 1777, 1166, 931, 437, 2086, 10, 972, 1460, 1618, 78, 787, 580, 734, 2177, 2014, 2323, 2376, 114, 577, 1441, 831, 2503, 2126, 1930, 2141, 1907, 1433, 1828, 2021, 713, 981, 1095, 684, 1259, 1932, 2314, 1617, 310, 1152, 394, 1367, 833, 1758, 2241, 1474, 73, 644, 1684, 1838, 424, 2337, 219, 1330, 276, 1319, 1472, 1074, 631, 1140, 2525, 1704, 882, 1285, 141, 1431, 2041, 2602, 1557, 920, 431, 280, 1388, 409, 784, 2423, 568, 714, 1185, 2482, 88, 1224, 770, 1532, 1603, 842, 338, 694, 586, 274, 1352, 1162, 766, 37, 223, 832, 165, 1271, 1374, 2487, 941, 1686, 296, 2275, 1180, 2270, 822, 2626, 2238, 1601, 434, 932, 281, 825, 1922, 20, 2440, 2027, 828, 2531, 1258, 400, 562, 2188, 1717, 1289, 147, 2034, 686, 1415, 2630, 1514, 1101, 146, 762, 1277, 1199, 727, 597, 578, 803, 2031, 987, 1411, 1706, 1062, 1746, 2452, 619, 401, 116, 249, 461, 1013, 2622, 910, 2631, 1811, 635, 172, 588, 2148, 2461, 656, 1368, 2492, 942, 227, 781, 1581, 1060, 183, 319, 1193, 340, 1880, 525, 538, 2237, 1239, 2635, 1195, 837, 2545, 2613, 1210, 2139, 1327, 2289, 740, 233, 2456, 257, 2370, 242, 1796, 2211, 625, 259, 1477, 1840, 1528, 1925, 408, 1794, 2586, 2533, 2087, 2413, 2312, 1256, 2510, 587, 2315, 2398, 250, 1743, 1304, 352, 2175, 384, 1924, 1773, 2403, 80, 2022, 2003, 2321, 1971, 200, 1125, 2406, 239, 1379, 1209, 1322, 653, 2244, 2227, 1213, 610, 1397, 2185, 2259, 2605, 1859, 123, 111, 702, 1386, 1414, 1990, 1967, 1287, 479, 1381, 2597, 840, 2462, 267, 1672, 1117, 1602, 1536, 1973, 188, 846, 2393, 260, 2140, 1030, 22, 951, 679, 124, 290, 2263, 1363, 173, 2039, 1607, 1394, 553, 1123, 221, 926, 1980, 2343, 1951, 2421, 647, 651, 1914, 1844, 1572, 103, 39, 1274, 1036, 458, 2223, 94, 1261, 2075, 1084, 1959, 1903, 1595, 1560, 1356, 1808, 447, 2368, 244, 1625, 404, 2254, 908, 1168, 418, 1231, 1633, 1491, 119, 2127, 1007, 757, 2, 2476, 990, 2336, 186, 50, 1975, 65, 184, 144, 1861, 312, 1837, 1375, 1454, 2469, 1851, 794, 2157, 8, 478, 2226, 650, 2571, 1963, 273, 925, 1926, 1518, 135, 1192, 291, 494, 1078, 2608, 2483, 2438, 82, 398, 177, 1134, 1056, 615, 1306, 301, 325, 902, 1251, 765, 982, 799, 2514, 865, 798, 743, 1619, 1588, 2231, 1044, 2316, 1977, 2629, 1917, 611, 2509, 847, 1530, 228, 2278, 373, 1317, 939, 2411, 2294, 2207, 547, 2535, 2636, 1276, 536, 1331, 671, 2458, 457, 530, 1865, 2410, 1752, 775, 1989, 2019, 90, 1593, 984, 2079, 1981, 118, 2251, 1961, 1173, 466, 379, 516, 573, 2446, 2607, 864, 1527, 1246, 1783, 1955, 800, 539, 1944, 128, 2319, 2561, 838, 1516, 973, 1681, 2069, 1355, 2295, 1544, 2592, 2618, 793, 2072, 1599, 637, 1972, 2515, 2397, 2637, 737, 442, 1407, 16, 456, 229, 701, 1676, 1377, 1138, 164, 2464, 2609, 93, 211, 1341, 2404, 1255, 382, 490, 24, 1455, 830, 1725, 1669, 851, 2297, 2394, 1652, 1767, 107, 877, 804, 1006, 1361, 907, 2603, 950, 1764, 2508, 17, 2427, 42, 2451, 1057, 693, 41, 2110, 1679, 2348, 253, 1818, 335, 1576, 1372, 978, 1463, 1011, 1344, 1754, 2255, 2152, 2236, 1055, 2103, 616, 2579, 1606, 2202, 2365, 2299, 315, 197, 795, 199, 1496, 2266, 2088, 2604, 2504, 1879, 614, 66, 1953, 1201, 52, 1534, 1589, 2497, 1561, 1147, 1802, 2059, 2032, 1591, 2595, 1085, 2589, 1227, 362, 1803, 2065, 782, 1385, 1801, 2182, 488, 474, 1786, 1399, 2381, 237, 2030, 1719, 2341, 1308, 1124, 1129, 68, 252, 1335, 983, 414, 824, 624, 791, 1723, 930, 2159, 2474, 320, 1050, 2285, 2417, 691, 1911, 1012, 2565, 2013, 419, 560, 2044, 663, 1600, 61, 2264, 1770, 2344, 2308, 1008, 789, 1281, 464, 2115, 189, 100, 528, 2162, 1233, 2481, 1692, 150, 307, 1269, 997, 2067, 1550, 109, 2620, 1234, 2351, 1449, 452, 937, 2036, 342, 2291, 2163, 675, 728, 1175, 1146, 265, 593, 1937, 445, 2048, 2049, 216, 2229, 331, 484, 1896, 1816, 1428, 2356, 1211, 271, 1660, 1240, 203, 2271, 558, 1351, 1745, 2377, 1812, 115, 155, 1735, 1590, 594, 2444, 1395, 1467, 827, 919, 2558, 1693, 788, 477, 2367, 1778, 2590, 1537, 630, 2239, 2008, 44, 636, 1473, 1065, 2147, 1711, 2384, 1960, 1222, 2143, 1021, 1141, 181, 2494, 2149, 759, 884, 344, 32, 2260, 1836, 2335, 1988, 645, 1155, 1678, 512, 599, 1086, 125, 985, 834, 1612, 59, 2537, 1002, 272, 450, 1490, 1952, 1965, 330, 1497, 2573, 491, 2095, 1187, 117, 1992, 1343, 2542, 321, 1750, 2425, 1284, 472, 112, 2301, 1035, 453, 33, 662, 1772, 1877, 1627, 389, 1122, 2002, 2142, 1511, 2085, 2434, 1225, 1889, 1082, 1430, 2372, 1639, 685, 54, 2465, 1165, 1004, 196, 1348, 2017, 1908, 1945, 2352, 2024, 1163, 542, 295, 856, 2400, 1874, 1310, 749, 551, 1695, 806, 163, 70, 641, 1406, 63, 589, 1046, 327, 646, 1316, 2382, 1186, 1790, 2167, 566, 1043, 1405, 870, 546, 2272, 2077, 1482, 1909, 286, 263, 928, 2252, 1413, 2524, 1587, 1207, 1018, 1194, 1235, 2061, 964, 1378, 643, 776, 190, 989, 1740, 2526, 1839, 2123, 2360, 1942, 1136, 160, 639, 869, 448, 1985, 169, 235, 1718, 2415, 2057, 1552, 1870, 2071, 571, 391, 1177, 220, 2200, 1031, 519, 1237, 2447, 1299, 304, 2552, 874, 1494, 2168, 192, 143, 1108, 2240, 1644, 1107, 1019, 1392, 668, 2084, 193, 1347, 1858, 2006, 2470, 1440, 2477, 153, 747, 2012, 377, 433, 843, 549, 1328, 1993, 91, 1805, 889, 892, 2191, 411, 2007, 572, 1831, 1675, 46, 2594, 1219, 592, 2128, 27, 149, 1025, 893, 2472, 97, 1439, 582, 602, 1712, 1653, 1964, 13, 279, 468, 1001, 1948, 1247, 2306, 1509, 341, 2092, 1845, 2513, 949, 1121, 402, 710, 1648, 2539, 813, 876, 1170, 9, 1757, 7, 499, 493, 1999, 575, 1150, 2284, 1615, 1938, 655, 2257, 2431, 1416, 626, 2484, 1897, 1815, 2161, 353, 1418, 2303, 1338, 202, 2612, 1901, 1987, 428, 455, 1793, 1336, 1501, 1654, 386, 1297, 1814, 2190, 251, 1668, 2292, 1795, 2473, 56, 241, 208, 366, 1731, 1592, 1582, 374, 126, 811, 285, 1244, 1058, 1554, 1628, 1854, 423, 305, 1312, 2418, 1079, 1292, 1088, 1824, 1087, 2346, 299, 812, 470, 2396, 1127, 1314, 2171, 72, 2230, 1850, 2453, 178, 999, 1217, 1747, 1144, 168, 517, 1647, 1713, 723, 2261, 2528, 2154, 277, 306, 933, 2129, 715, 1096, 507, 1427, 773, 1984, 83, 725, 1566, 826, 462, 1894, 245, 444, 1403, 2324, 2164, 854, 888, 2339, 191, 2416, 1863, 1810, 552, 454, 1538, 1876, 2498, 2449, 2364, 732, 360, 2063, 938, 2054, 1848, 2569, 19, 145, 970, 993, 1610, 2615, 1766, 482, 1051, 550, 742, 323, 1064, 835, 2478, 1212, 1525, 1313, 2090, 677, 1267, 2436, 1702, 1453, 2277, 521, 1779, 2265, 1248, 1856, 2378, 1539, 527, 140, 2204, 736, 1729, 642, 314, 48, 2234, 969, 689, 1041, 738, 1634, 2042, 722, 657, 1324, 2632, 1918, 1875, 1093, 2310, 1358, 1184, 750, 1614, 1028, 498, 1475, 649, 672, 664, 1099, 979, 1242, 268, 1563, 1800, 1522, 2468, 1423, 1827, 1657, 1663, 1860, 2541, 1565, 1179, 287, 1505, 1226, 238, 2429, 1105, 2144, 2543, 555, 2502, 1830, 758, 15, 1788, 284, 1362, 2269, 683, 2262, 988, 2215, 1404, 1283, 1435, 1819, 1291, 1450, 1200, 1325, 967, 75, 1809, 2225, 2130, 1077, 607, 839, 2522, 965, 959, 1556, 1857, 2056, 2359, 511, 2585, 2217, 1584, 2559, 2334, 2247, 1422, 180, 1298, 369, 836, 1934, 1957, 1373, 532, 2553, 815, 2614, 1424, 1133, 2455, 2015, 2392, 1114, 74, 2332, 324, 347, 1320, 1899, 465, 612, 818, 1936, 896, 1216, 2026, 904, 1739, 264, 1488, 1268, 1559, 2530, 1275, 508, 1510, 1089, 2076, 209, 2155, 2380, 1562, 1476, 504, 85, 2040, 1543, 2201, 777, 1943, 2250, 801, 1156, 2023, 1891, 1437, 608, 2035, 853, 1154, 1384, 297, 673, 731, 1898, 489, 1900, 2322, 1080, 278, 313, 1158, 1635, 1852, 213, 392, 1190, 1643, 905, 2018, 879, 2011, 1549, 1596, 21, 670, 2327, 2548, 1642, 476, 1128, 2405, 2485, 2118, 1223, 1789, 1293, 929, 1575, 1761, 432, 1906, 735, 980, 2395, 1260, 1389, 2430, 1682, 381, 2584, 2422, 1721, 1032, 1882, 2576, 2199, 1998, 696, 294, 2385, 1483, 2577, 975, 1115, 1398, 1426, 1366, 2375, 891, 1696, 1513, 1650, 817, 1512, 676, 911, 1189, 435, 105, 1756, 459, 1010, 2611, 669, 303, 1694, 298, 1181, 1853, 2283, 1023, 1515, 2599, 2578, 2051, 224, 1817, 2053, 699, 881, 1480, 2235, 541, 2209, 1434, 179, 410, 682, 240, 600, 2038, 1203, 1822, 634, 2020, 12, 1749, 2253, 609, 2575, 2633, 958, 1846, 1143, 1871, 1730, 1329, 2432, 2232, 396, 11, 821, 1820, 407, 746, 2216, 1178, 152, 2610, 2245, 77, 363, 2490, 194, 1149, 976, 1976, 176, 524, 585, 522, 1919, 802, 1636, 698, 2628, 1369, 741, 2045, 996, 1401, 92, 998, 1110, 1886, 1279, 1608, 2108, 1318, 2373, 372, 480, 2387, 2330, 895, 703, 501, 2532, 1578, 2135, 1069, 1847, 719, 632, 1895, 855, 754, 309, 1956, 554, 1033, 963, 1662, 0, 906, 1045, 142, 771, 2179, 785, 2566, 1939, 2165, 1469, 903, 923, 2320, 1842, 1499, 87, 606, 1791, 515, 1598, 1470, 1504, 2098, 1962, 1888, 2457, 705, 1295, 661, 2556, 357, 1947, 1826, 849, 1927, 2184, 894, 1229, 212, 380, 1047, 64, 823, 1139, 1221, 210, 1493, 2112, 23, 375, 2512, 1705, 2379, 1383, 934, 1111, 166, 2104]);

    map
};

pub(super) static LOWER_NAME_TO_IDX: PhfMap<[u8], u16> = {
    let Ok(map) = PhfMap::new(10656210946825422025, &[(0, 63), (0, 7), (0, 174), (0, 120), (0, 54), (0, 27), (0, 129), (0, 3), (0, 6), (0, 5), (0, 47), (0, 267), (0, 27), (0, 121), (0, 0), (0, 41), (0, 23), (0, 177), (0, 0), (0, 549), (0, 19), (0, 82), (0, 48), (0, 74), (0, 0), (0, 28), (0, 130), (0, 0), (0, 47), (0, 120), (0, 311), (0, 135), (0, 0), (0, 653), (0, 3), (0, 0), (0, 41), (0, 76), (0, 23), (0, 2), (0, 0), (0, 102), (0, 385), (0, 279), (0, 1124), (0, 47), (0, 0), (0, 364), (0, 23), (0, 41), (0, 19), (0, 60), (0, 549), (0, 14), (0, 63), (0, 3), (0, 50), (0, 28), (0, 4), (0, 382), (0, 199), (0, 4), (0, 66), (0, 296), (0, 115), (0, 148), (0, 1), (0, 12), (0, 100), (0, 33), (0, 15), (0, 5), (0, 872), (0, 18), (0, 121), (0, 267), (0, 17), (0, 1), (0, 48), (0, 0), (0, 9), (0, 100), (0, 1207), (0, 557), (0, 0), (0, 11), (0, 4), (0, 340), (0, 0), (0, 1), (0, 368), (0, 28), (0, 168), (0, 5), (0, 23), (0, 20), (0, 44), (0, 8), (0, 800), (0, 27), (0, 0), (0, 0), (0, 0), (0, 2), (0, 271), (0, 309), (0, 5), (0, 0), (0, 28), (0, 211), (0, 1), (0, 470), (0, 28), (0, 2), (0, 89), (0, 21), (0, 14), (0, 56), (0, 1506), (0, 3), (0, 11), (0, 513), (0, 61), (0, 7), (0, 0), (0, 7), (0, 263), (0, 34), (0, 203), (0, 855), (0, 1), (0, 36), (0, 148), (0, 0), (0, 341), (0, 432), (0, 226), (0, 1755), (0, 141), (0, 103), (0, 166), (0, 22), (0, 0), (0, 79), (0, 1), (0, 1197), (0, 3), (0, 112), (0, 35), (0, 0), (0, 232), (0, 42), (0, 17), (0, 604), (0, 214), (0, 1), (0, 15), (0, 0), (0, 49), (0, 538), (0, 11), (0, 44), (0, 70), (0, 4), (0, 12), (0, 2), (0, 45), (0, 23), (0, 59), (0, 1), (0, 219), (0, 84), (0, 962), (0, 96), (0, 196), (0, 244), (0, 988), (0, 1325), (0, 15), (0, 5), (0, 145), (0, 56), (0, 52), (0, 188), (0, 798), (0, 1), (0, 2), (0, 2), (0, 0), (0, 818), (0, 1), (0, 201), (0, 6), (0, 81), (0, 415), (0, 29), (0, 1), (0, 7), (0, 60), (0, 0), (0, 659), (1, 0), (0, 4), (0, 2), (0, 18), (0, 82), (0, 0), (0, 5), (0, 0), (0, 1), (0, 0), (0, 58), (0, 496), (0, 45), (0, 90), (0, 17), (0, 133), (0, 32), (0, 10), (0, 0), (0, 109), (0, 125), (0, 30), (0, 1), (0, 1315), (0, 2), (0, 483), (0, 3), (0, 3), (0, 490), (0, 363), (0, 0), (0, 0), (0, 323), (0, 182), (0, 89), (0, 11), (0, 2), (0, 9), (0, 0), (0, 2), (0, 520), (0, 87), (0, 232), (0, 561), (0, 12), (0, 1619), (0, 13), (0, 1), (0, 27), (0, 336), (0, 10), (0, 3), (0, 2), (0, 36), (0, 607), (0, 67), (0, 214), (0, 1), (0, 28), (0, 5), (0, 158), (0, 246), (0, 0), (0, 1047), (0, 24), (0, 857), (0, 194), (0, 97), (0, 10), (0, 369), (0, 0), (0, 47), (0, 5), (0, 251), (0, 39), (0, 7), (0, 0), (0, 5), (0, 8), (0, 98), (0, 708), (0, 0), (0, 17), (0, 33), (1, 1009), (0, 543), (0, 138), (0, 136), (0, 516), (0, 38), (0, 1), (0, 874), (0, 2), (0, 308), (1, 894), (0, 3), (0, 0), (0, 107), (0, 436), (0, 1068), (0, 1116), (0, 156), (0, 11), (0, 4), (0, 28), (0, 1430), (0, 6), (0, 23), (0, 1313), (0, 36), (0, 18), (0, 12), (0, 804), (0, 1075), (0, 54), (1, 228), (0, 160), (0, 1948), (0, 2220), (0, 1), (0, 67), (0, 53), (0, 1087), (0, 3), (0, 100), (0, 1), (0, 1834), (0, 0), (1, 2067), (0, 26), (0, 508), (0, 1808), (0, 1), (0, 33), (0, 185), (0, 649), (0, 0), (0, 15), (1, 372), (0, 516), (0, 2135), (0, 16), (0, 8), (0, 464), (0, 3), (0, 4), (0, 141), (1, 844), (0, 684), (0, 0), (0, 212), (0, 8), (0, 0), (0, 18), (0, 224), (0, 3), (0, 10), (0, 1895), (2, 318), (0, 686), (0, 774), (0, 74), (1, 1297), (1, 2), (0, 623), (0, 12), (0, 26), (0, 6), (0, 40), (0, 1170), (0, 159), (0, 62), (0, 1783), (0, 17), (0, 7), (0, 13), (0, 0), (0, 0), (0, 1), (0, 18), (0, 20), (0, 492), (0, 635), (0, 18), (0, 145), (0, 1019), (0, 972), (0, 10), (1, 111), (0, 134), (0, 0), (0, 108), (0, 0), (0, 15), (0, 1366), (0, 18), (0, 268), (0, 101), (0, 195), (0, 118), (0, 18), (0, 116), (0, 6), (0, 31), (0, 32), (0, 8), (0, 0), (0, 3), (1, 1166), (0, 561), (0, 1482), (0, 0), (0, 19), (0, 19), (0, 13), (0, 81), (0, 147), (0, 21), (0, 50), (0, 66), (1, 29), (0, 1), (0, 311), (0, 191), (0, 102), (0, 63), (0, 98), (0, 1), (2, 160), (0, 3), (0, 11), (0, 12), (0, 126), (0, 966), (0, 0), (1, 1014), (0, 209), (1, 705), (0, 1125), (0, 1081), (0, 892), (9, 2125), (0, 1), (0, 3), (0, 371), (0, 580), (0, 0), (2, 1444), (1, 325), (0, 9), (0, 23), (0, 16), (7, 21), (0, 0), (0, 200), (0, 0), (0, 1133), (1, 1635), (0, 120), (0, 834), (0, 1001), (1, 491), (0, 1062), (1, 879), (0, 39), (0, 1), (3, 529), (0, 19), (2, 1302), (0, 1657), (0, 42), (0, 189), (0, 1859), (5, 1426), (1, 1588), (0, 899), (0, 1704), (0, 199), (3, 415), (0, 1651), (0, 0), (7, 383), (0, 922), (0, 120), (0, 0), (0, 257), (0, 0), (1, 1149), (0, 5), (0, 1161), (0, 1621), (0, 1), (0, 1121), (9, 104), (0, 576), (0, 197), (0, 2118), (3, 2205), (0, 471), (0, 635), (10, 283), (0, 11), (5, 2272), (0, 122), (0, 2), (0, 123), (9, 1854), (0, 732), (0, 0), (9, 1557), (0, 8)], &[627, 903, 426, 1971, 1585, 1073, 1221, 1720, 1145, 1303, 289, 840, 380, 1689, 1644, 1157, 432, 260, 1591, 1193, 1006, 189, 1220, 1062, 7, 973, 1791, 665, 1722, 950, 487, 2016, 1140, 1123, 216, 1165, 2292, 2336, 2000, 1660, 743, 11, 954, 677, 1423, 1079, 445, 1516, 2637, 667, 1099, 1647, 75, 1403, 2584, 1980, 1182, 190, 2419, 1824, 998, 682, 1514, 2037, 2254, 1341, 639, 494, 2162, 1805, 785, 1537, 2534, 953, 444, 2325, 2459, 2118, 1658, 1735, 1291, 670, 1912, 197, 2525, 2438, 413, 2007, 452, 693, 1259, 1121, 126, 1023, 369, 1527, 2106, 1864, 1795, 98, 888, 1498, 2313, 334, 1529, 2371, 644, 762, 250, 1311, 2161, 1296, 2296, 801, 2126, 1301, 393, 1281, 1945, 395, 952, 723, 1269, 111, 1342, 2341, 2536, 657, 2034, 986, 1267, 1607, 2012, 1060, 1631, 628, 1011, 1239, 1982, 2286, 1916, 2136, 1261, 193, 2476, 2360, 2565, 727, 1793, 278, 313, 770, 2600, 2618, 2457, 344, 442, 2474, 1180, 740, 699, 2442, 1503, 121, 1086, 1739, 1601, 1771, 2522, 2063, 347, 689, 2252, 901, 621, 28, 2465, 2319, 958, 1078, 1143, 2250, 2122, 2511, 904, 2613, 1202, 125, 795, 1710, 1652, 1340, 1045, 255, 1975, 1228, 2375, 265, 983, 2225, 492, 1664, 2176, 919, 2251, 408, 1150, 767, 1103, 1227, 1989, 1584, 961, 2615, 1246, 401, 1863, 1351, 1593, 2406, 787, 1054, 1106, 1248, 1238, 1112, 103, 1075, 2033, 267, 698, 2513, 1025, 396, 2440, 1773, 2369, 2616, 1846, 1201, 361, 2468, 1928, 905, 1513, 2008, 2464, 648, 1181, 1830, 424, 2074, 2426, 2291, 675, 383, 623, 1053, 2595, 1055, 1708, 897, 405, 2576, 224, 712, 1936, 175, 2144, 1251, 1126, 1927, 2100, 1094, 2500, 1840, 951, 679, 2418, 629, 429, 2244, 2535, 1328, 738, 1022, 2035, 1335, 92, 1637, 279, 674, 2198, 1539, 1072, 343, 1309, 124, 1545, 2393, 2322, 2492, 372, 398, 2445, 2273, 169, 2018, 889, 848, 1077, 1932, 1344, 2557, 645, 1937, 1042, 1902, 765, 1264, 2301, 2542, 2248, 414, 2043, 1371, 769, 2295, 1993, 61, 2333, 291, 1854, 635, 2611, 1396, 1587, 2247, 1972, 1757, 2117, 2524, 676, 632, 634, 1548, 2447, 2599, 758, 2045, 2276, 2597, 1226, 13, 1507, 1564, 2624, 896, 1440, 1560, 281, 2370, 404, 1751, 1302, 2269, 1730, 1285, 1033, 177, 1749, 1922, 654, 1996, 69, 1941, 1986, 256, 838, 416, 1399, 2166, 411, 702, 1426, 1610, 1405, 1450, 605, 113, 328, 1290, 931, 2056, 106, 1206, 1728, 90, 948, 668, 450, 858, 2589, 2417, 1365, 19, 489, 1029, 2227, 1359, 2293, 2130, 1783, 2147, 2530, 249, 1966, 886, 1096, 503, 1910, 2585, 1380, 2086, 498, 1383, 2131, 213, 1554, 1392, 2289, 827, 2150, 1646, 1969, 2108, 2142, 2388, 128, 1198, 332, 275, 1899, 1811, 1675, 1605, 2094, 430, 176, 1322, 288, 703, 1917, 357, 2308, 957, 2145, 1420, 614, 1692, 1035, 1555, 130, 89, 2394, 2265, 1338, 360, 2242, 16, 2326, 865, 1561, 2627, 1136, 65, 2207, 1904, 1397, 964, 560, 1703, 330, 2427, 2240, 1686, 1817, 2098, 822, 2171, 2441, 2567, 1004, 1502, 2622, 1630, 912, 2448, 1098, 2396, 839, 1988, 2625, 923, 2487, 864, 617, 243, 913, 685, 987, 1881, 508, 1617, 2515, 1196, 615, 1544, 1801, 739, 1571, 2229, 2547, 1015, 2491, 2561, 1016, 1497, 1195, 748, 1411, 1048, 1843, 2302, 2545, 1586, 2374, 1014, 1373, 559, 244, 1994, 2272, 1961, 1232, 1231, 510, 2496, 2190, 2103, 312, 1313, 1297, 1141, 1531, 1090, 1068, 1826, 1521, 509, 651, 202, 752, 1639, 1034, 2436, 1329, 1726, 2231, 438, 761, 2377, 1649, 1597, 2335, 1506, 780, 400, 1230, 2066, 376, 2422, 917, 1082, 1110, 1056, 2090, 72, 2310, 974, 994, 86, 2416, 1701, 925, 1873, 633, 1953, 1211, 1368, 2221, 2467, 2570, 2437, 73, 131, 364, 1049, 1018, 841, 1361, 132, 2451, 2566, 2629, 1914, 2199, 2626, 2149, 1595, 319, 2391, 729, 2246, 434, 2404, 1172, 962, 1523, 2606, 1223, 2550, 1159, 1938, 1915, 2278, 895, 2461, 1253, 1352, 2559, 963, 804, 2093, 2031, 1909, 2359, 730, 2021, 906, 2330, 898, 1321, 1312, 1330, 778, 821, 2539, 1382, 2091, 2256, 355, 2631, 2243, 1670, 378, 2315, 1925, 1666, 2452, 853, 1856, 900, 2446, 2124, 662, 1604, 1645, 725, 1084, 1847, 1705, 247, 2119, 1755, 2587, 17, 327, 1753, 754, 1951, 697, 1541, 598, 565, 1681, 1093, 1013, 346, 1515, 1370, 1262, 2070, 2529, 2053, 2297, 1889, 1973, 371, 2348, 1876, 506, 435, 1277, 1128, 2495, 1327, 2379, 1557, 507, 2519, 2234, 1008, 122, 454, 1074, 1508, 2501, 2158, 1120, 556, 1496, 1244, 1225, 257, 793, 2526, 1551, 2477, 1654, 2512, 1500, 1575, 2395, 263, 381, 782, 2303, 2486, 1602, 696, 1797, 2381, 611, 652, 1855, 929, 813, 181, 718, 1656, 625, 388, 1669, 436, 241, 2279, 2516, 1549, 2554, 918, 1199, 944, 1286, 2125, 687, 1204, 25, 2610, 187, 1935, 31, 2282, 2215, 1337, 2355, 860, 708, 1859, 1298, 2157, 116, 817, 1218, 940, 1559, 893, 2551, 1845, 2101, 600, 27, 2357, 2217, 2608, 317, 894, 1002, 32, 563, 2632, 2083, 66, 558, 862, 2636, 81, 102, 78, 2556, 1759, 796, 2197, 2001, 548, 1921, 1657, 2517, 688, 391, 1363, 2170, 911, 1964, 970, 1636, 870, 1308, 1398, 1907, 1662, 2392, 2061, 2087, 1036, 2390, 1332, 1012, 315, 1809, 1378, 356, 1350, 2175, 173, 1386, 916, 403, 616, 2484, 1512, 934, 1588, 850, 1424, 2097, 18, 1665, 2208, 1282, 1641, 2298, 85, 2398, 253, 2378, 2358, 1672, 774, 637, 2195, 1393, 1724, 709, 252, 1952, 711, 2141, 1704, 1, 2017, 869, 1691, 428, 2537, 2592, 1236, 1421, 1648, 2582, 690, 618, 1930, 2630, 2044, 1877, 501, 2135, 2510, 909, 1535, 96, 1908, 825, 671, 2114, 2455, 969, 1651, 788, 2428, 2167, 1137, 1438, 68, 200, 1192, 2423, 2005, 1634, 715, 691, 1741, 641, 713, 1682, 1177, 1884, 1769, 907, 2594, 1349, 199, 423, 2397, 1169, 831, 1828, 1875, 410, 2431, 1323, 2039, 2288, 673, 737, 1668, 188, 826, 1976, 1063, 1924, 2193, 1590, 1416, 2367, 1040, 493, 1360, 1965, 810, 2621, 2069, 439, 180, 722, 1319, 1428, 62, 2462, 2460, 1579, 2169, 1409, 829, 2400, 2082, 1898, 1390, 1305, 1950, 488, 2572, 1903, 705, 2407, 333, 1697, 1718, 872, 812, 659, 392, 2062, 2546, 1224, 777, 1955, 1495, 2399, 2342, 402, 1815, 1067, 881, 120, 2506, 1623, 1574, 794, 1240, 1064, 1362, 1933, 2245, 1021, 2268, 91, 482, 2480, 1867, 1003, 2347, 1242, 759, 1842, 2494, 2612, 415, 2259, 341, 2059, 484, 1374, 307, 1043, 1339, 2068, 756, 808, 1105, 2604, 1572, 561, 965, 2178, 266, 1208, 1387, 24, 325, 77, 2060, 370, 2509, 1679, 2143, 661, 1991, 2275, 1611, 1614, 185, 1653, 2518, 1052, 1300, 1070, 878, 2054, 1934, 2514, 2481, 2504, 1807, 1273, 1448, 2073, 2316, 1650, 309, 1625, 1667, 14, 716, 1603, 1538, 1655, 2323, 87, 760, 2307, 1085, 1320, 1821, 2334, 2067, 2084, 2337, 775, 1395, 1624, 1347, 2163, 1031, 649, 274, 2580, 1714, 2317, 1188, 859, 921, 365, 1284, 681, 1245, 2055, 308, 956, 1217, 2429, 1777, 295, 2593, 194, 1324, 399, 118, 1839, 2578, 1589, 2521, 222, 1505, 1528, 1853, 119, 1176, 2274, 1289, 1186, 1640, 1292, 1057, 2194, 345, 1138, 2263, 2538, 2280, 1168, 842, 443, 843, 1827, 2057, 2077, 1357, 2531, 875, 1716, 485, 2088, 619, 936, 1304, 2112, 1044, 1556, 828, 2152, 2, 927, 499, 2078, 2133, 2596, 2579, 2581, 1316, 610, 100, 1142, 2408, 3, 976, 2270, 1896, 2607, 2255, 2331, 1974, 2555, 891, 1848, 2314, 1897, 2591, 1894, 2258, 1255, 2277, 1578, 1163, 650, 1234, 1178, 985, 358, 2430, 2187, 1942, 1862, 946, 978, 1761, 320, 2092, 2623, 172, 1293, 1174, 855, 2253, 1114, 1097, 2168, 1166, 1032, 658, 2520, 108, 1963, 602, 786, 2502, 1929, 741, 2015, 324, 655, 157, 1888, 192, 1119, 2192, 2349, 757, 1065, 1954, 1949, 2373, 557, 2424, 2165, 1763, 622, 1066, 449, 115, 1987, 2320, 2201, 2173, 2004, 214, 1676, 2071, 2222, 1835, 726, 832, 2449, 2328, 1868, 892, 1058, 920, 1179, 553, 1173, 1367, 1878, 2470, 922, 1620, 930, 2366, 2204, 1836, 1358, 1454, 2385, 2344, 1265, 1622, 2305, 1315, 1968, 932, 2261, 1524, 1534, 791, 310, 1442, 1643, 1831, 2363, 1047, 746, 1155, 1673, 2435, 422, 2635, 797, 2214, 1041, 873, 1532, 1275, 178, 2115, 390, 933, 2283, 2294, 1425, 104, 562, 1272, 851, 1118, 1413, 1019, 2499, 669, 988, 2339, 766, 710, 1215, 4, 915, 276, 2365, 1958, 322, 293, 2350, 2619, 1844, 2080, 607, 71, 753, 1765, 1943, 368, 750, 683, 1499, 992, 1813, 2189, 2372, 1333, 5, 2351, 1871, 1633, 597, 991, 1642, 1583, 684, 721, 1530, 1258, 2038, 1419, 363, 789, 593, 1612, 2505, 2105, 1979, 1091, 631, 2203, 2266, 447, 2154, 2563, 824, 1837, 490, 2013, 2076, 1092, 101, 2183, 1517, 1536, 2128, 2025, 678, 606, 2137, 2140, 2032, 1037, 820, 437, 603, 2216, 773, 694, 349, 1737, 218, 2064, 1852, 666, 123, 890, 195, 1874, 2586, 638, 502, 2177, 1279, 2425, 979, 2186, 1926, 2011, 2127, 2598, 924, 1632, 1882, 613, 1998, 1997, 1394, 2483, 1684, 1947, 1706, 1214, 1696, 811, 1866, 883, 1348, 1107, 2020, 1695, 1257, 876, 2009, 2568, 1028, 646, 926, 2065, 1880, 1089, 1213, 2237, 348, 2541, 863, 433, 2603, 183, 2281, 1030, 1626, 1596, 2444, 420, 1000, 1569, 397, 857, 2290, 382, 1789, 1346, 2560, 2356, 643, 1967, 2434, 802, 2111, 2376, 389, 1446, 595, 1156, 1132, 0, 1911, 2553, 1294, 724, 384, 599, 26, 2085, 2413, 798, 1372, 2609, 1981, 594, 2386, 1833, 2601, 1027, 1785, 591, 2048, 2340, 1678, 1088, 2220, 2309, 254, 2306, 1295, 747, 1247, 2552, 1102, 366, 2235, 29, 2304, 505, 1430, 2409, 2479, 2051, 1698, 809, 1683, 835, 942, 74, 1823, 2634, 1832, 264, 2287, 2352, 1674, 2414, 1331, 1456, 1712, 2571, 1187, 1117, 1891, 2079, 867, 2389, 1134, 642, 856, 1960, 1051, 1920, 2181, 943, 1369, 2104, 980, 719, 412, 640, 2472, 2095, 220, 182, 2324, 2420, 771, 2558, 2412, 2006, 342, 1185, 2129, 2123, 2439, 2072, 1702, 99, 2200, 1288, 1552, 2134, 612, 1111, 2102, 2402, 1270, 1307, 1343, 1024, 2109, 2223, 1356, 899, 129, 656, 280, 1109, 1825, 10, 672, 318, 1865, 314, 441, 97, 171, 1553, 1940, 1197, 834, 776, 1619, 1779, 1995, 1162, 2107, 968, 871, 292, 636, 1299, 1207, 1573, 734, 1948, 783, 1567, 1627, 2405, 1158, 1613, 2155, 495, 874, 647, 1787, 2602, 1621, 1962, 1857, 479, 1799, 455, 1767, 1688, 1900, 1550, 1893, 406, 306, 706, 1125, 2014, 1355, 1325, 2346, 112, 1618, 1418, 221, 2160, 849, 692, 486, 1237, 554, 592, 2493, 1210, 2174, 2411, 1104, 2146, 735, 2262, 2311, 1547, 2527, 1263, 1977, 1901, 1005, 1913, 601, 1287, 772, 937, 448, 379, 1061, 2226, 1522, 866, 1080, 1685, 608, 127, 984, 64, 2249, 311, 170, 2113, 1381, 242, 833, 1108, 1026, 2184, 2469, 1540, 2180, 2453, 790, 847, 1609, 1918, 425, 2380, 417, 814, 2026, 799, 2209, 1191, 1171, 2564, 105, 1385, 2528, 1526, 982, 1887, 972, 1733, 1939, 1519, 882, 359, 419, 1444, 2361, 386, 996, 367, 1101, 2463, 2410, 2271, 837, 720, 1999, 251, 1212, 1598, 2353, 2614, 1146, 1543, 2415, 1095, 1693, 440, 999, 2202, 114, 1849, 1905, 2299, 1170, 792, 1083, 2489, 1233, 326, 2211, 2003, 1194, 1671, 728, 707, 1364, 1638, 1376, 1978, 1345, 2401, 1518, 88, 908, 2153, 1525, 1254, 63, 1071, 219, 2383, 2219, 1278, 555, 2224, 1069, 1310, 2300, 803, 198, 1563, 966, 20, 2503, 2478, 564, 2022, 1391, 1161, 2110, 1354, 805, 1690, 2148, 362, 977, 1241, 880, 660, 2368, 196, 2182, 2605, 186, 939, 248, 2473, 630, 1038, 844, 1501, 1100, 2164, 2575, 2466, 316, 1576, 1565, 2421, 1050, 15, 407, 768, 1377, 2139, 744, 2027, 2050, 480, 431, 70, 1959, 2159, 2046, 483, 453, 1432, 2138, 2132, 742, 1250, 1116, 1890, 84, 2052, 1059, 902, 1906, 1616, 1076, 2343, 1130, 914, 2196, 2228, 2047, 1841, 779, 2096, 2327, 2019, 1266, 2028, 941, 421, 2238, 2433, 2354, 387, 215, 1183, 1694, 1167, 819, 1113, 928, 1249, 2002, 2583, 1434, 497, 1546, 481, 1992, 331, 1216, 1334, 1838, 1829, 1775, 1260, 626, 1317, 1318, 2617, 2030, 1271, 997, 1581, 755, 352, 2260, 885, 340, 1388, 456, 2588, 2544, 1209, 949, 1046, 884, 1190, 1436, 823, 1879, 500, 2029, 1680, 277, 1219, 816, 350, 2490, 990, 1803, 1205, 2081, 1274, 764, 2338, 989, 2590, 733, 800, 1366, 2041, 2024, 1087, 1122, 2284, 664, 947, 1677, 1200, 110, 1600, 1870, 1663, 1203, 2089, 1985, 323, 2230, 2236, 714, 93, 751, 9, 2458, 1243, 2403, 620, 861, 246, 1520, 887, 1822, 1511, 836, 717, 167, 1819, 784, 815, 2042, 1594, 1659, 1401, 1189, 2218, 2454, 1280, 1389, 1153, 184, 2285, 1375, 2482, 496, 2010, 854, 1628, 2058, 217, 504, 2485, 191, 1886, 23, 680, 2172, 2075, 2387, 2488, 168, 749, 731, 1314, 174, 2562, 2241, 1533, 2040, 1582, 1001, 1700, 2577, 596, 1990, 1861, 2540, 21, 1384, 1115, 2156, 701, 1892, 1608, 2188, 80, 609, 1687, 2364, 409, 2036, 1010, 935, 1017, 2508, 1984, 2185, 2471, 1268, 1509, 1850, 704, 2532, 1745, 1020, 2120, 1336, 981, 2497, 83, 2321, 2116, 1422, 624, 2233, 1164, 179, 663, 30, 975, 1615, 1353, 830, 427, 1858, 1851, 290, 781, 2049, 351, 1592, 995, 321, 2332, 653, 2213, 33, 1743, 1883, 879, 245, 971, 1124, 1566, 700, 2345, 2548, 1306, 1635, 959, 2121, 2232, 2179, 2432, 2450, 2212, 736, 806, 2329, 1510, 1504, 2443, 223, 2533, 2569, 2523, 2498, 2023, 1256, 1956, 418, 1957, 1452, 2264, 1570, 763, 2475, 852, 2543, 2205, 2633, 1860, 2210, 79, 2257, 2267, 1276, 1580, 1568, 938, 745, 1606, 1081, 117, 1931, 212, 1039, 22, 686, 967, 1919, 1229, 1222, 1160, 2573, 2456, 109, 1970, 377, 385, 818, 94, 2206, 6, 1872, 695, 1699, 1144, 329, 1629, 107, 1599, 910, 1661, 446, 1885, 394, 868, 2362, 732, 845, 846, 1147, 1577, 76, 1379, 2574, 2239, 960, 2382, 8, 12, 945, 67, 1252, 82, 1834, 1175, 451, 1326, 1781, 2384, 1923, 2620, 1983, 807, 1283, 34, 2318, 1558, 1869, 491, 955, 604, 2549, 2312, 1235, 1944, 1946, 2151, 2191, 1542, 1154, 95, 877, 993, 1184, 1895, 1747, 2099, 1562, 2628]);

    map
};

pub(super) static KEYSYM_TO_UPPER_KEYSYM: PhfMap<u32, KeysymCaseMapping> = {
    let Ok(map) = PhfMap::new(10656210946825422025, &[(0, 0), (0, 39), (0, 0), (0, 0), (0, 3), (0, 3), (0, 7), (0, 0), (0, 1), (0, 24), (0, 10), (0, 5), (0, 0), (0, 5), (0, 6), (0, 68), (0, 0), (0, 19), (0, 1), (0, 15), (0, 2), (0, 34), (0, 38), (0, 3), (0, 11), (0, 95), (0, 3), (0, 1), (0, 9), (0, 4), (0, 6), (0, 103), (0, 1), (0, 1), (0, 6), (0, 0), (0, 77), (0, 0), (0, 89), (0, 23), (0, 2), (0, 0), (0, 0), (0, 99), (0, 0), (0, 7), (0, 18), (0, 145), (0, 2), (0, 64), (0, 13), (0, 6), (0, 31), (0, 82), (0, 0), (0, 0), (0, 37), (0, 16), (0, 0), (0, 151), (0, 54), (0, 6), (3, 50), (0, 6)], &[KeysymCaseMapping { keysym: 0x06d8, other: 0x000006f8 }, KeysymCaseMapping { keysym: 0x03b3, other: 0x000003a3 }, KeysymCaseMapping { keysym: 0x06ae, other: 0x000006be }, KeysymCaseMapping { keysym: 0x01ea, other: 0x000001ca }, KeysymCaseMapping { keysym: 0x07e1, other: 0x000007c1 }, KeysymCaseMapping { keysym: 0x06c5, other: 0x000006e5 }, KeysymCaseMapping { keysym: 0x07e5, other: 0x000007c5 }, KeysymCaseMapping { keysym: 0x02fd, other: 0x000002dd }, KeysymCaseMapping { keysym: 0x06cb, other: 0x000006eb }, KeysymCaseMapping { keysym: 0x06d0, other: 0x000006f0 }, KeysymCaseMapping { keysym: 0x06c4, other: 0x000006e4 }, KeysymCaseMapping { keysym: 0x03b5, other: 0x000003a5 }, KeysymCaseMapping { keysym: 0x01b5, other: 0x000001a5 }, KeysymCaseMapping { keysym: 0x00fc, other: 0x000000dc }, KeysymCaseMapping { keysym: 0x06a3, other: 0x000006b3 }, KeysymCaseMapping { keysym: 0x06d7, other: 0x000006f7 }, KeysymCaseMapping { keysym: 0x00f3, other: 0x000000d3 }, KeysymCaseMapping { keysym: 0x01fe, other: 0x000001de }, KeysymCaseMapping { keysym: 0x06ac, other: 0x000006bc }, KeysymCaseMapping { keysym: 0x07e2, other: 0x000007c2 }, KeysymCaseMapping { keysym: 0x06c6, other: 0x000006e6 }, KeysymCaseMapping { keysym: 0x06a5, other: 0x000006b5 }, KeysymCaseMapping { keysym: 0x01bb, other: 0x000001ab }, KeysymCaseMapping { keysym: 0x01e8, other: 0x000001c8 }, KeysymCaseMapping { keysym: 0x01f1, other: 0x000001d1 }, KeysymCaseMapping { keysym: 0x07f3, other: 0x000007d2 }, KeysymCaseMapping { keysym: 0x00ef, other: 0x000000cf }, KeysymCaseMapping { keysym: 0x06a1, other: 0x000006b1 }, KeysymCaseMapping { keysym: 0x06d5, other: 0x000006f5 }, KeysymCaseMapping { keysym: 0x00f1, other: 0x000000d1 }, KeysymCaseMapping { keysym: 0x06c9, other: 0x000006e9 }, KeysymCaseMapping { keysym: 0x06aa, other: 0x000006ba }, KeysymCaseMapping { keysym: 0x03bc, other: 0x000003ac }, KeysymCaseMapping { keysym: 0x00fa, other: 0x000000da }, KeysymCaseMapping { keysym: 0x03e7, other: 0x000003c7 }, KeysymCaseMapping { keysym: 0x00e0, other: 0x000000c0 }, KeysymCaseMapping { keysym: 0x00e1, other: 0x000000c1 }, KeysymCaseMapping { keysym: 0x07ef, other: 0x000007cf }, KeysymCaseMapping { keysym: 0x00ed, other: 0x000000cd }, KeysymCaseMapping { keysym: 0x07f1, other: 0x000007d1 }, KeysymCaseMapping { keysym: 0x07e4, other: 0x000007c4 }, KeysymCaseMapping { keysym: 0x03f9, other: 0x000003d9 }, KeysymCaseMapping { keysym: 0x06a4, other: 0x000006b4 }, KeysymCaseMapping { keysym: 0x07b9, other: 0x000007a9 }, KeysymCaseMapping { keysym: 0x02bc, other: 0x000002ac }, KeysymCaseMapping { keysym: 0x01b6, other: 0x000001a6 }, KeysymCaseMapping { keysym: 0x02b1, other: 0x000002a1 }, KeysymCaseMapping { keysym: 0x06d3, other: 0x000006f3 }, KeysymCaseMapping { keysym: 0x01b1, other: 0x000001a1 }, KeysymCaseMapping { keysym: 0x02e5, other: 0x000002c5 }, KeysymCaseMapping { keysym: 0x01bc, other: 0x000001ac }, KeysymCaseMapping { keysym: 0x02f8, other: 0x000002d8 }, KeysymCaseMapping { keysym: 0x00f8, other: 0x000000d8 }, KeysymCaseMapping { keysym: 0x00ee, other: 0x000000ce }, KeysymCaseMapping { keysym: 0x01f8, other: 0x000001d8 }, KeysymCaseMapping { keysym: 0x07b4, other: 0x000007a4 }, KeysymCaseMapping { keysym: 0x06a8, other: 0x000006b8 }, KeysymCaseMapping { keysym: 0x00eb, other: 0x000000cb }, KeysymCaseMapping { keysym: 0x07e9, other: 0x000007c9 }, KeysymCaseMapping { keysym: 0x00ff, other: 0x000013be }, KeysymCaseMapping { keysym: 0x01fb, other: 0x000001db }, KeysymCaseMapping { keysym: 0x07eb, other: 0x000007cb }, KeysymCaseMapping { keysym: 0x03bf, other: 0x000003bd }, KeysymCaseMapping { keysym: 0x06ad, other: 0x000006bd }, KeysymCaseMapping { keysym: 0x06c2, other: 0x000006e2 }, KeysymCaseMapping { keysym: 0x00fd, other: 0x000000dd }, KeysymCaseMapping { keysym: 0x07ee, other: 0x000007ce }, KeysymCaseMapping { keysym: 0x01e6, other: 0x000001c6 }, KeysymCaseMapping { keysym: 0x01b3, other: 0x000001a3 }, KeysymCaseMapping { keysym: 0x07b2, other: 0x000007a2 }, KeysymCaseMapping { keysym: 0x00e6, other: 0x000000c6 }, KeysymCaseMapping { keysym: 0x06c0, other: 0x000006e0 }, KeysymCaseMapping { keysym: 0x02e6, other: 0x000002c6 }, KeysymCaseMapping { keysym: 0x00ec, other: 0x000000cc }, KeysymCaseMapping { keysym: 0x06d1, other: 0x000006f1 }, KeysymCaseMapping { keysym: 0x00f9, other: 0x000000d9 }, KeysymCaseMapping { keysym: 0x03ec, other: 0x000003cc }, KeysymCaseMapping { keysym: 0x01f5, other: 0x000001d5 }, KeysymCaseMapping { keysym: 0x00df, other: 0x01001e9e }, KeysymCaseMapping { keysym: 0x07f7, other: 0x000007d7 }, KeysymCaseMapping { keysym: 0x02f5, other: 0x000002d5 }, KeysymCaseMapping { keysym: 0x03fe, other: 0x000003de }, KeysymCaseMapping { keysym: 0x07b7, other: 0x000007a7 }, KeysymCaseMapping { keysym: 0x06ab, other: 0x000006bb }, KeysymCaseMapping { keysym: 0x06d2, other: 0x000006f2 }, KeysymCaseMapping { keysym: 0x00fb, other: 0x000000db }, KeysymCaseMapping { keysym: 0x03ba, other: 0x000003aa }, KeysymCaseMapping { keysym: 0x07b8, other: 0x000007a8 }, KeysymCaseMapping { keysym: 0x06df, other: 0x000006ff }, KeysymCaseMapping { keysym: 0x06ce, other: 0x000006ee }, KeysymCaseMapping { keysym: 0x07e8, other: 0x000007c8 }, KeysymCaseMapping { keysym: 0x06a6, other: 0x000006b6 }, KeysymCaseMapping { keysym: 0x00e4, other: 0x000000c4 }, KeysymCaseMapping { keysym: 0x00f6, other: 0x000000d6 }, KeysymCaseMapping { keysym: 0x07ed, other: 0x000007cd }, KeysymCaseMapping { keysym: 0x00ea, other: 0x000000ca }, KeysymCaseMapping { keysym: 0x07ec, other: 0x000007cc }, KeysymCaseMapping { keysym: 0x07bb, other: 0x000007ab }, KeysymCaseMapping { keysym: 0x00b5, other: 0x000007cc }, KeysymCaseMapping { keysym: 0x01f9, other: 0x000001d9 }, KeysymCaseMapping { keysym: 0x02bb, other: 0x000002ab }, KeysymCaseMapping { keysym: 0x01be, other: 0x000001ae }, KeysymCaseMapping { keysym: 0x07f9, other: 0x000007d9 }, KeysymCaseMapping { keysym: 0x03ef, other: 0x000003cf }, KeysymCaseMapping { keysym: 0x06dd, other: 0x000006fd }, KeysymCaseMapping { keysym: 0x07f6, other: 0x000007d6 }, KeysymCaseMapping { keysym: 0x00e2, other: 0x000000c2 }, KeysymCaseMapping { keysym: 0x03f2, other: 0x000003d2 }, KeysymCaseMapping { keysym: 0x06af, other: 0x000006bf }, KeysymCaseMapping { keysym: 0x03b6, other: 0x000003a6 }, KeysymCaseMapping { keysym: 0x07ea, other: 0x000007ca }, KeysymCaseMapping { keysym: 0x06c3, other: 0x000006e3 }, KeysymCaseMapping { keysym: 0x00f4, other: 0x000000d4 }, KeysymCaseMapping { keysym: 0x03f1, other: 0x000003d1 }, KeysymCaseMapping { keysym: 0x02b6, other: 0x000002a6 }, KeysymCaseMapping { keysym: 0x07b5, other: 0x000007a5 }, KeysymCaseMapping { keysym: 0x06a9, other: 0x000006b9 }, KeysymCaseMapping { keysym: 0x01ef, other: 0x000001cf }, KeysymCaseMapping { keysym: 0x06db, other: 0x000006fb }, KeysymCaseMapping { keysym: 0x07f4, other: 0x000007d4 }, KeysymCaseMapping { keysym: 0x07e7, other: 0x000007c7 }, KeysymCaseMapping { keysym: 0x06de, other: 0x000006fe }, KeysymCaseMapping { keysym: 0x03e0, other: 0x000003c0 }, KeysymCaseMapping { keysym: 0x01b9, other: 0x000001a9 }, KeysymCaseMapping { keysym: 0x07b3, other: 0x000007a3 }, KeysymCaseMapping { keysym: 0x06c1, other: 0x000006e1 }, KeysymCaseMapping { keysym: 0x01e0, other: 0x000001c0 }, KeysymCaseMapping { keysym: 0x01f2, other: 0x000001d2 }, KeysymCaseMapping { keysym: 0x00e9, other: 0x000000c9 }, KeysymCaseMapping { keysym: 0x00e7, other: 0x000000c7 }, KeysymCaseMapping { keysym: 0x00e8, other: 0x000000c8 }, KeysymCaseMapping { keysym: 0x06ca, other: 0x000006ea }, KeysymCaseMapping { keysym: 0x03bb, other: 0x000003ab }, KeysymCaseMapping { keysym: 0x02b9, other: 0x00000049 }, KeysymCaseMapping { keysym: 0x06dc, other: 0x000006fc }, KeysymCaseMapping { keysym: 0x07f2, other: 0x000007d2 }, KeysymCaseMapping { keysym: 0x00e5, other: 0x000000c5 }, KeysymCaseMapping { keysym: 0x01ba, other: 0x000001aa }, KeysymCaseMapping { keysym: 0x06d9, other: 0x000006f9 }, KeysymCaseMapping { keysym: 0x00f2, other: 0x000000d2 }, KeysymCaseMapping { keysym: 0x06cf, other: 0x000006ef }, KeysymCaseMapping { keysym: 0x06cc, other: 0x000006ec }, KeysymCaseMapping { keysym: 0x07f8, other: 0x000007d8 }, KeysymCaseMapping { keysym: 0x06c8, other: 0x000006e8 }, KeysymCaseMapping { keysym: 0x13bd, other: 0x000013bc }, KeysymCaseMapping { keysym: 0x03fd, other: 0x000003dd }, KeysymCaseMapping { keysym: 0x01f0, other: 0x000001d0 }, KeysymCaseMapping { keysym: 0x06a7, other: 0x000006b7 }, KeysymCaseMapping { keysym: 0x06d4, other: 0x000006f4 }, KeysymCaseMapping { keysym: 0x07f0, other: 0x000007d0 }, KeysymCaseMapping { keysym: 0x01bf, other: 0x000001af }, KeysymCaseMapping { keysym: 0x06a2, other: 0x000006b2 }, KeysymCaseMapping { keysym: 0x01e5, other: 0x000001c5 }, KeysymCaseMapping { keysym: 0x00f0, other: 0x000000d0 }, KeysymCaseMapping { keysym: 0x00e3, other: 0x000000c3 }, KeysymCaseMapping { keysym: 0x01ec, other: 0x000001cc }, KeysymCaseMapping { keysym: 0x07e6, other: 0x000007c6 }, KeysymCaseMapping { keysym: 0x06c7, other: 0x000006e7 }, KeysymCaseMapping { keysym: 0x06cd, other: 0x000006ed }, KeysymCaseMapping { keysym: 0x07b1, other: 0x000007a1 }, KeysymCaseMapping { keysym: 0x00f5, other: 0x000000d5 }, KeysymCaseMapping { keysym: 0x02fe, other: 0x000002de }, KeysymCaseMapping { keysym: 0x07e3, other: 0x000007c3 }, KeysymCaseMapping { keysym: 0x01e3, other: 0x000001c3 }, KeysymCaseMapping { keysym: 0x06da, other: 0x000006fa }, KeysymCaseMapping { keysym: 0x07f5, other: 0x000007d5 }, KeysymCaseMapping { keysym: 0x06d6, other: 0x000006f6 }, KeysymCaseMapping { keysym: 0x03f3, other: 0x000003d3 }, KeysymCaseMapping { keysym: 0x08f6, other: 0x01000191 }, KeysymCaseMapping { keysym: 0x00fe, other: 0x000000de }]);

    map
};

pub(super) static KEYSYM_TO_CHAR: &[KeysymChar] = &[
    KeysymChar {
        keysym: 0x0020,
        char: ' ',
    },
    KeysymChar {
        keysym: 0x0021,
        char: '!',
    },
    KeysymChar {
        keysym: 0x0022,
        char: '"',
    },
    KeysymChar {
        keysym: 0x0023,
        char: '#',
    },
    KeysymChar {
        keysym: 0x0024,
        char: '$',
    },
    KeysymChar {
        keysym: 0x0025,
        char: '%',
    },
    KeysymChar {
        keysym: 0x0026,
        char: '&',
    },
    KeysymChar {
        keysym: 0x0027,
        char: '\'',
    },
    KeysymChar {
        keysym: 0x0028,
        char: '(',
    },
    KeysymChar {
        keysym: 0x0029,
        char: ')',
    },
    KeysymChar {
        keysym: 0x002a,
        char: '*',
    },
    KeysymChar {
        keysym: 0x002b,
        char: '+',
    },
    KeysymChar {
        keysym: 0x002c,
        char: ',',
    },
    KeysymChar {
        keysym: 0x002d,
        char: '-',
    },
    KeysymChar {
        keysym: 0x002e,
        char: '.',
    },
    KeysymChar {
        keysym: 0x002f,
        char: '/',
    },
    KeysymChar {
        keysym: 0x0030,
        char: '0',
    },
    KeysymChar {
        keysym: 0x0031,
        char: '1',
    },
    KeysymChar {
        keysym: 0x0032,
        char: '2',
    },
    KeysymChar {
        keysym: 0x0033,
        char: '3',
    },
    KeysymChar {
        keysym: 0x0034,
        char: '4',
    },
    KeysymChar {
        keysym: 0x0035,
        char: '5',
    },
    KeysymChar {
        keysym: 0x0036,
        char: '6',
    },
    KeysymChar {
        keysym: 0x0037,
        char: '7',
    },
    KeysymChar {
        keysym: 0x0038,
        char: '8',
    },
    KeysymChar {
        keysym: 0x0039,
        char: '9',
    },
    KeysymChar {
        keysym: 0x003a,
        char: ':',
    },
    KeysymChar {
        keysym: 0x003b,
        char: ';',
    },
    KeysymChar {
        keysym: 0x003c,
        char: '<',
    },
    KeysymChar {
        keysym: 0x003d,
        char: '=',
    },
    KeysymChar {
        keysym: 0x003e,
        char: '>',
    },
    KeysymChar {
        keysym: 0x003f,
        char: '?',
    },
    KeysymChar {
        keysym: 0x0040,
        char: '@',
    },
    KeysymChar {
        keysym: 0x0041,
        char: 'A',
    },
    KeysymChar {
        keysym: 0x0042,
        char: 'B',
    },
    KeysymChar {
        keysym: 0x0043,
        char: 'C',
    },
    KeysymChar {
        keysym: 0x0044,
        char: 'D',
    },
    KeysymChar {
        keysym: 0x0045,
        char: 'E',
    },
    KeysymChar {
        keysym: 0x0046,
        char: 'F',
    },
    KeysymChar {
        keysym: 0x0047,
        char: 'G',
    },
    KeysymChar {
        keysym: 0x0048,
        char: 'H',
    },
    KeysymChar {
        keysym: 0x0049,
        char: 'I',
    },
    KeysymChar {
        keysym: 0x004a,
        char: 'J',
    },
    KeysymChar {
        keysym: 0x004b,
        char: 'K',
    },
    KeysymChar {
        keysym: 0x004c,
        char: 'L',
    },
    KeysymChar {
        keysym: 0x004d,
        char: 'M',
    },
    KeysymChar {
        keysym: 0x004e,
        char: 'N',
    },
    KeysymChar {
        keysym: 0x004f,
        char: 'O',
    },
    KeysymChar {
        keysym: 0x0050,
        char: 'P',
    },
    KeysymChar {
        keysym: 0x0051,
        char: 'Q',
    },
    KeysymChar {
        keysym: 0x0052,
        char: 'R',
    },
    KeysymChar {
        keysym: 0x0053,
        char: 'S',
    },
    KeysymChar {
        keysym: 0x0054,
        char: 'T',
    },
    KeysymChar {
        keysym: 0x0055,
        char: 'U',
    },
    KeysymChar {
        keysym: 0x0056,
        char: 'V',
    },
    KeysymChar {
        keysym: 0x0057,
        char: 'W',
    },
    KeysymChar {
        keysym: 0x0058,
        char: 'X',
    },
    KeysymChar {
        keysym: 0x0059,
        char: 'Y',
    },
    KeysymChar {
        keysym: 0x005a,
        char: 'Z',
    },
    KeysymChar {
        keysym: 0x005b,
        char: '[',
    },
    KeysymChar {
        keysym: 0x005c,
        char: '\\',
    },
    KeysymChar {
        keysym: 0x005d,
        char: ']',
    },
    KeysymChar {
        keysym: 0x005e,
        char: '^',
    },
    KeysymChar {
        keysym: 0x005f,
        char: '_',
    },
    KeysymChar {
        keysym: 0x0060,
        char: '`',
    },
    KeysymChar {
        keysym: 0x0061,
        char: 'a',
    },
    KeysymChar {
        keysym: 0x0062,
        char: 'b',
    },
    KeysymChar {
        keysym: 0x0063,
        char: 'c',
    },
    KeysymChar {
        keysym: 0x0064,
        char: 'd',
    },
    KeysymChar {
        keysym: 0x0065,
        char: 'e',
    },
    KeysymChar {
        keysym: 0x0066,
        char: 'f',
    },
    KeysymChar {
        keysym: 0x0067,
        char: 'g',
    },
    KeysymChar {
        keysym: 0x0068,
        char: 'h',
    },
    KeysymChar {
        keysym: 0x0069,
        char: 'i',
    },
    KeysymChar {
        keysym: 0x006a,
        char: 'j',
    },
    KeysymChar {
        keysym: 0x006b,
        char: 'k',
    },
    KeysymChar {
        keysym: 0x006c,
        char: 'l',
    },
    KeysymChar {
        keysym: 0x006d,
        char: 'm',
    },
    KeysymChar {
        keysym: 0x006e,
        char: 'n',
    },
    KeysymChar {
        keysym: 0x006f,
        char: 'o',
    },
    KeysymChar {
        keysym: 0x0070,
        char: 'p',
    },
    KeysymChar {
        keysym: 0x0071,
        char: 'q',
    },
    KeysymChar {
        keysym: 0x0072,
        char: 'r',
    },
    KeysymChar {
        keysym: 0x0073,
        char: 's',
    },
    KeysymChar {
        keysym: 0x0074,
        char: 't',
    },
    KeysymChar {
        keysym: 0x0075,
        char: 'u',
    },
    KeysymChar {
        keysym: 0x0076,
        char: 'v',
    },
    KeysymChar {
        keysym: 0x0077,
        char: 'w',
    },
    KeysymChar {
        keysym: 0x0078,
        char: 'x',
    },
    KeysymChar {
        keysym: 0x0079,
        char: 'y',
    },
    KeysymChar {
        keysym: 0x007a,
        char: 'z',
    },
    KeysymChar {
        keysym: 0x007b,
        char: '{',
    },
    KeysymChar {
        keysym: 0x007c,
        char: '|',
    },
    KeysymChar {
        keysym: 0x007d,
        char: '}',
    },
    KeysymChar {
        keysym: 0x007e,
        char: '~',
    },
    KeysymChar {
        keysym: 0x00a0,
        char: '\u{a0}',
    },
    KeysymChar {
        keysym: 0x00a1,
        char: '¡',
    },
    KeysymChar {
        keysym: 0x00a2,
        char: '¢',
    },
    KeysymChar {
        keysym: 0x00a3,
        char: '£',
    },
    KeysymChar {
        keysym: 0x00a4,
        char: '¤',
    },
    KeysymChar {
        keysym: 0x00a5,
        char: '¥',
    },
    KeysymChar {
        keysym: 0x00a6,
        char: '¦',
    },
    KeysymChar {
        keysym: 0x00a7,
        char: '§',
    },
    KeysymChar {
        keysym: 0x00a8,
        char: '¨',
    },
    KeysymChar {
        keysym: 0x00a9,
        char: '©',
    },
    KeysymChar {
        keysym: 0x00aa,
        char: 'ª',
    },
    KeysymChar {
        keysym: 0x00ab,
        char: '«',
    },
    KeysymChar {
        keysym: 0x00ac,
        char: '¬',
    },
    KeysymChar {
        keysym: 0x00ad,
        char: '\u{ad}',
    },
    KeysymChar {
        keysym: 0x00ae,
        char: '®',
    },
    KeysymChar {
        keysym: 0x00af,
        char: '¯',
    },
    KeysymChar {
        keysym: 0x00b0,
        char: '°',
    },
    KeysymChar {
        keysym: 0x00b1,
        char: '±',
    },
    KeysymChar {
        keysym: 0x00b2,
        char: '²',
    },
    KeysymChar {
        keysym: 0x00b3,
        char: '³',
    },
    KeysymChar {
        keysym: 0x00b4,
        char: '´',
    },
    KeysymChar {
        keysym: 0x00b5,
        char: 'µ',
    },
    KeysymChar {
        keysym: 0x00b6,
        char: '¶',
    },
    KeysymChar {
        keysym: 0x00b7,
        char: '·',
    },
    KeysymChar {
        keysym: 0x00b8,
        char: '¸',
    },
    KeysymChar {
        keysym: 0x00b9,
        char: '¹',
    },
    KeysymChar {
        keysym: 0x00ba,
        char: 'º',
    },
    KeysymChar {
        keysym: 0x00bb,
        char: '»',
    },
    KeysymChar {
        keysym: 0x00bc,
        char: '¼',
    },
    KeysymChar {
        keysym: 0x00bd,
        char: '½',
    },
    KeysymChar {
        keysym: 0x00be,
        char: '¾',
    },
    KeysymChar {
        keysym: 0x00bf,
        char: '¿',
    },
    KeysymChar {
        keysym: 0x00c0,
        char: 'À',
    },
    KeysymChar {
        keysym: 0x00c1,
        char: 'Á',
    },
    KeysymChar {
        keysym: 0x00c2,
        char: 'Â',
    },
    KeysymChar {
        keysym: 0x00c3,
        char: 'Ã',
    },
    KeysymChar {
        keysym: 0x00c4,
        char: 'Ä',
    },
    KeysymChar {
        keysym: 0x00c5,
        char: 'Å',
    },
    KeysymChar {
        keysym: 0x00c6,
        char: 'Æ',
    },
    KeysymChar {
        keysym: 0x00c7,
        char: 'Ç',
    },
    KeysymChar {
        keysym: 0x00c8,
        char: 'È',
    },
    KeysymChar {
        keysym: 0x00c9,
        char: 'É',
    },
    KeysymChar {
        keysym: 0x00ca,
        char: 'Ê',
    },
    KeysymChar {
        keysym: 0x00cb,
        char: 'Ë',
    },
    KeysymChar {
        keysym: 0x00cc,
        char: 'Ì',
    },
    KeysymChar {
        keysym: 0x00cd,
        char: 'Í',
    },
    KeysymChar {
        keysym: 0x00ce,
        char: 'Î',
    },
    KeysymChar {
        keysym: 0x00cf,
        char: 'Ï',
    },
    KeysymChar {
        keysym: 0x00d0,
        char: 'Ð',
    },
    KeysymChar {
        keysym: 0x00d1,
        char: 'Ñ',
    },
    KeysymChar {
        keysym: 0x00d2,
        char: 'Ò',
    },
    KeysymChar {
        keysym: 0x00d3,
        char: 'Ó',
    },
    KeysymChar {
        keysym: 0x00d4,
        char: 'Ô',
    },
    KeysymChar {
        keysym: 0x00d5,
        char: 'Õ',
    },
    KeysymChar {
        keysym: 0x00d6,
        char: 'Ö',
    },
    KeysymChar {
        keysym: 0x00d7,
        char: '×',
    },
    KeysymChar {
        keysym: 0x00d8,
        char: 'Ø',
    },
    KeysymChar {
        keysym: 0x00d9,
        char: 'Ù',
    },
    KeysymChar {
        keysym: 0x00da,
        char: 'Ú',
    },
    KeysymChar {
        keysym: 0x00db,
        char: 'Û',
    },
    KeysymChar {
        keysym: 0x00dc,
        char: 'Ü',
    },
    KeysymChar {
        keysym: 0x00dd,
        char: 'Ý',
    },
    KeysymChar {
        keysym: 0x00de,
        char: 'Þ',
    },
    KeysymChar {
        keysym: 0x00df,
        char: 'ß',
    },
    KeysymChar {
        keysym: 0x00e0,
        char: 'à',
    },
    KeysymChar {
        keysym: 0x00e1,
        char: 'á',
    },
    KeysymChar {
        keysym: 0x00e2,
        char: 'â',
    },
    KeysymChar {
        keysym: 0x00e3,
        char: 'ã',
    },
    KeysymChar {
        keysym: 0x00e4,
        char: 'ä',
    },
    KeysymChar {
        keysym: 0x00e5,
        char: 'å',
    },
    KeysymChar {
        keysym: 0x00e6,
        char: 'æ',
    },
    KeysymChar {
        keysym: 0x00e7,
        char: 'ç',
    },
    KeysymChar {
        keysym: 0x00e8,
        char: 'è',
    },
    KeysymChar {
        keysym: 0x00e9,
        char: 'é',
    },
    KeysymChar {
        keysym: 0x00ea,
        char: 'ê',
    },
    KeysymChar {
        keysym: 0x00eb,
        char: 'ë',
    },
    KeysymChar {
        keysym: 0x00ec,
        char: 'ì',
    },
    KeysymChar {
        keysym: 0x00ed,
        char: 'í',
    },
    KeysymChar {
        keysym: 0x00ee,
        char: 'î',
    },
    KeysymChar {
        keysym: 0x00ef,
        char: 'ï',
    },
    KeysymChar {
        keysym: 0x00f0,
        char: 'ð',
    },
    KeysymChar {
        keysym: 0x00f1,
        char: 'ñ',
    },
    KeysymChar {
        keysym: 0x00f2,
        char: 'ò',
    },
    KeysymChar {
        keysym: 0x00f3,
        char: 'ó',
    },
    KeysymChar {
        keysym: 0x00f4,
        char: 'ô',
    },
    KeysymChar {
        keysym: 0x00f5,
        char: 'õ',
    },
    KeysymChar {
        keysym: 0x00f6,
        char: 'ö',
    },
    KeysymChar {
        keysym: 0x00f7,
        char: '÷',
    },
    KeysymChar {
        keysym: 0x00f8,
        char: 'ø',
    },
    KeysymChar {
        keysym: 0x00f9,
        char: 'ù',
    },
    KeysymChar {
        keysym: 0x00fa,
        char: 'ú',
    },
    KeysymChar {
        keysym: 0x00fb,
        char: 'û',
    },
    KeysymChar {
        keysym: 0x00fc,
        char: 'ü',
    },
    KeysymChar {
        keysym: 0x00fd,
        char: 'ý',
    },
    KeysymChar {
        keysym: 0x00fe,
        char: 'þ',
    },
    KeysymChar {
        keysym: 0x00ff,
        char: 'ÿ',
    },
    KeysymChar {
        keysym: 0x01a1,
        char: 'Ą',
    },
    KeysymChar {
        keysym: 0x01a2,
        char: '˘',
    },
    KeysymChar {
        keysym: 0x01a3,
        char: 'Ł',
    },
    KeysymChar {
        keysym: 0x01a5,
        char: 'Ľ',
    },
    KeysymChar {
        keysym: 0x01a6,
        char: 'Ś',
    },
    KeysymChar {
        keysym: 0x01a9,
        char: 'Š',
    },
    KeysymChar {
        keysym: 0x01aa,
        char: 'Ş',
    },
    KeysymChar {
        keysym: 0x01ab,
        char: 'Ť',
    },
    KeysymChar {
        keysym: 0x01ac,
        char: 'Ź',
    },
    KeysymChar {
        keysym: 0x01ae,
        char: 'Ž',
    },
    KeysymChar {
        keysym: 0x01af,
        char: 'Ż',
    },
    KeysymChar {
        keysym: 0x01b1,
        char: 'ą',
    },
    KeysymChar {
        keysym: 0x01b2,
        char: '˛',
    },
    KeysymChar {
        keysym: 0x01b3,
        char: 'ł',
    },
    KeysymChar {
        keysym: 0x01b5,
        char: 'ľ',
    },
    KeysymChar {
        keysym: 0x01b6,
        char: 'ś',
    },
    KeysymChar {
        keysym: 0x01b7,
        char: 'ˇ',
    },
    KeysymChar {
        keysym: 0x01b9,
        char: 'š',
    },
    KeysymChar {
        keysym: 0x01ba,
        char: 'ş',
    },
    KeysymChar {
        keysym: 0x01bb,
        char: 'ť',
    },
    KeysymChar {
        keysym: 0x01bc,
        char: 'ź',
    },
    KeysymChar {
        keysym: 0x01bd,
        char: '˝',
    },
    KeysymChar {
        keysym: 0x01be,
        char: 'ž',
    },
    KeysymChar {
        keysym: 0x01bf,
        char: 'ż',
    },
    KeysymChar {
        keysym: 0x01c0,
        char: 'Ŕ',
    },
    KeysymChar {
        keysym: 0x01c3,
        char: 'Ă',
    },
    KeysymChar {
        keysym: 0x01c5,
        char: 'Ĺ',
    },
    KeysymChar {
        keysym: 0x01c6,
        char: 'Ć',
    },
    KeysymChar {
        keysym: 0x01c8,
        char: 'Č',
    },
    KeysymChar {
        keysym: 0x01ca,
        char: 'Ę',
    },
    KeysymChar {
        keysym: 0x01cc,
        char: 'Ě',
    },
    KeysymChar {
        keysym: 0x01cf,
        char: 'Ď',
    },
    KeysymChar {
        keysym: 0x01d0,
        char: 'Đ',
    },
    KeysymChar {
        keysym: 0x01d1,
        char: 'Ń',
    },
    KeysymChar {
        keysym: 0x01d2,
        char: 'Ň',
    },
    KeysymChar {
        keysym: 0x01d5,
        char: 'Ő',
    },
    KeysymChar {
        keysym: 0x01d8,
        char: 'Ř',
    },
    KeysymChar {
        keysym: 0x01d9,
        char: 'Ů',
    },
    KeysymChar {
        keysym: 0x01db,
        char: 'Ű',
    },
    KeysymChar {
        keysym: 0x01de,
        char: 'Ţ',
    },
    KeysymChar {
        keysym: 0x01e0,
        char: 'ŕ',
    },
    KeysymChar {
        keysym: 0x01e3,
        char: 'ă',
    },
    KeysymChar {
        keysym: 0x01e5,
        char: 'ĺ',
    },
    KeysymChar {
        keysym: 0x01e6,
        char: 'ć',
    },
    KeysymChar {
        keysym: 0x01e8,
        char: 'č',
    },
    KeysymChar {
        keysym: 0x01ea,
        char: 'ę',
    },
    KeysymChar {
        keysym: 0x01ec,
        char: 'ě',
    },
    KeysymChar {
        keysym: 0x01ef,
        char: 'ď',
    },
    KeysymChar {
        keysym: 0x01f0,
        char: 'đ',
    },
    KeysymChar {
        keysym: 0x01f1,
        char: 'ń',
    },
    KeysymChar {
        keysym: 0x01f2,
        char: 'ň',
    },
    KeysymChar {
        keysym: 0x01f5,
        char: 'ő',
    },
    KeysymChar {
        keysym: 0x01f8,
        char: 'ř',
    },
    KeysymChar {
        keysym: 0x01f9,
        char: 'ů',
    },
    KeysymChar {
        keysym: 0x01fb,
        char: 'ű',
    },
    KeysymChar {
        keysym: 0x01fe,
        char: 'ţ',
    },
    KeysymChar {
        keysym: 0x01ff,
        char: '˙',
    },
    KeysymChar {
        keysym: 0x02a1,
        char: 'Ħ',
    },
    KeysymChar {
        keysym: 0x02a6,
        char: 'Ĥ',
    },
    KeysymChar {
        keysym: 0x02a9,
        char: 'İ',
    },
    KeysymChar {
        keysym: 0x02ab,
        char: 'Ğ',
    },
    KeysymChar {
        keysym: 0x02ac,
        char: 'Ĵ',
    },
    KeysymChar {
        keysym: 0x02b1,
        char: 'ħ',
    },
    KeysymChar {
        keysym: 0x02b6,
        char: 'ĥ',
    },
    KeysymChar {
        keysym: 0x02b9,
        char: 'ı',
    },
    KeysymChar {
        keysym: 0x02bb,
        char: 'ğ',
    },
    KeysymChar {
        keysym: 0x02bc,
        char: 'ĵ',
    },
    KeysymChar {
        keysym: 0x02c5,
        char: 'Ċ',
    },
    KeysymChar {
        keysym: 0x02c6,
        char: 'Ĉ',
    },
    KeysymChar {
        keysym: 0x02d5,
        char: 'Ġ',
    },
    KeysymChar {
        keysym: 0x02d8,
        char: 'Ĝ',
    },
    KeysymChar {
        keysym: 0x02dd,
        char: 'Ŭ',
    },
    KeysymChar {
        keysym: 0x02de,
        char: 'Ŝ',
    },
    KeysymChar {
        keysym: 0x02e5,
        char: 'ċ',
    },
    KeysymChar {
        keysym: 0x02e6,
        char: 'ĉ',
    },
    KeysymChar {
        keysym: 0x02f5,
        char: 'ġ',
    },
    KeysymChar {
        keysym: 0x02f8,
        char: 'ĝ',
    },
    KeysymChar {
        keysym: 0x02fd,
        char: 'ŭ',
    },
    KeysymChar {
        keysym: 0x02fe,
        char: 'ŝ',
    },
    KeysymChar {
        keysym: 0x03a2,
        char: 'ĸ',
    },
    KeysymChar {
        keysym: 0x03a3,
        char: 'Ŗ',
    },
    KeysymChar {
        keysym: 0x03a5,
        char: 'Ĩ',
    },
    KeysymChar {
        keysym: 0x03a6,
        char: 'Ļ',
    },
    KeysymChar {
        keysym: 0x03aa,
        char: 'Ē',
    },
    KeysymChar {
        keysym: 0x03ab,
        char: 'Ģ',
    },
    KeysymChar {
        keysym: 0x03ac,
        char: 'Ŧ',
    },
    KeysymChar {
        keysym: 0x03b3,
        char: 'ŗ',
    },
    KeysymChar {
        keysym: 0x03b5,
        char: 'ĩ',
    },
    KeysymChar {
        keysym: 0x03b6,
        char: 'ļ',
    },
    KeysymChar {
        keysym: 0x03ba,
        char: 'ē',
    },
    KeysymChar {
        keysym: 0x03bb,
        char: 'ģ',
    },
    KeysymChar {
        keysym: 0x03bc,
        char: 'ŧ',
    },
    KeysymChar {
        keysym: 0x03bd,
        char: 'Ŋ',
    },
    KeysymChar {
        keysym: 0x03bf,
        char: 'ŋ',
    },
    KeysymChar {
        keysym: 0x03c0,
        char: 'Ā',
    },
    KeysymChar {
        keysym: 0x03c7,
        char: 'Į',
    },
    KeysymChar {
        keysym: 0x03cc,
        char: 'Ė',
    },
    KeysymChar {
        keysym: 0x03cf,
        char: 'Ī',
    },
    KeysymChar {
        keysym: 0x03d1,
        char: 'Ņ',
    },
    KeysymChar {
        keysym: 0x03d2,
        char: 'Ō',
    },
    KeysymChar {
        keysym: 0x03d3,
        char: 'Ķ',
    },
    KeysymChar {
        keysym: 0x03d9,
        char: 'Ų',
    },
    KeysymChar {
        keysym: 0x03dd,
        char: 'Ũ',
    },
    KeysymChar {
        keysym: 0x03de,
        char: 'Ū',
    },
    KeysymChar {
        keysym: 0x03e0,
        char: 'ā',
    },
    KeysymChar {
        keysym: 0x03e7,
        char: 'į',
    },
    KeysymChar {
        keysym: 0x03ec,
        char: 'ė',
    },
    KeysymChar {
        keysym: 0x03ef,
        char: 'ī',
    },
    KeysymChar {
        keysym: 0x03f1,
        char: 'ņ',
    },
    KeysymChar {
        keysym: 0x03f2,
        char: 'ō',
    },
    KeysymChar {
        keysym: 0x03f3,
        char: 'ķ',
    },
    KeysymChar {
        keysym: 0x03f9,
        char: 'ų',
    },
    KeysymChar {
        keysym: 0x03fd,
        char: 'ũ',
    },
    KeysymChar {
        keysym: 0x03fe,
        char: 'ū',
    },
    KeysymChar {
        keysym: 0x047e,
        char: '‾',
    },
    KeysymChar {
        keysym: 0x04a1,
        char: '。',
    },
    KeysymChar {
        keysym: 0x04a2,
        char: '「',
    },
    KeysymChar {
        keysym: 0x04a3,
        char: '」',
    },
    KeysymChar {
        keysym: 0x04a4,
        char: '、',
    },
    KeysymChar {
        keysym: 0x04a5,
        char: '・',
    },
    KeysymChar {
        keysym: 0x04a6,
        char: 'ヲ',
    },
    KeysymChar {
        keysym: 0x04a7,
        char: 'ァ',
    },
    KeysymChar {
        keysym: 0x04a8,
        char: 'ィ',
    },
    KeysymChar {
        keysym: 0x04a9,
        char: 'ゥ',
    },
    KeysymChar {
        keysym: 0x04aa,
        char: 'ェ',
    },
    KeysymChar {
        keysym: 0x04ab,
        char: 'ォ',
    },
    KeysymChar {
        keysym: 0x04ac,
        char: 'ャ',
    },
    KeysymChar {
        keysym: 0x04ad,
        char: 'ュ',
    },
    KeysymChar {
        keysym: 0x04ae,
        char: 'ョ',
    },
    KeysymChar {
        keysym: 0x04af,
        char: 'ッ',
    },
    KeysymChar {
        keysym: 0x04b0,
        char: 'ー',
    },
    KeysymChar {
        keysym: 0x04b1,
        char: 'ア',
    },
    KeysymChar {
        keysym: 0x04b2,
        char: 'イ',
    },
    KeysymChar {
        keysym: 0x04b3,
        char: 'ウ',
    },
    KeysymChar {
        keysym: 0x04b4,
        char: 'エ',
    },
    KeysymChar {
        keysym: 0x04b5,
        char: 'オ',
    },
    KeysymChar {
        keysym: 0x04b6,
        char: 'カ',
    },
    KeysymChar {
        keysym: 0x04b7,
        char: 'キ',
    },
    KeysymChar {
        keysym: 0x04b8,
        char: 'ク',
    },
    KeysymChar {
        keysym: 0x04b9,
        char: 'ケ',
    },
    KeysymChar {
        keysym: 0x04ba,
        char: 'コ',
    },
    KeysymChar {
        keysym: 0x04bb,
        char: 'サ',
    },
    KeysymChar {
        keysym: 0x04bc,
        char: 'シ',
    },
    KeysymChar {
        keysym: 0x04bd,
        char: 'ス',
    },
    KeysymChar {
        keysym: 0x04be,
        char: 'セ',
    },
    KeysymChar {
        keysym: 0x04bf,
        char: 'ソ',
    },
    KeysymChar {
        keysym: 0x04c0,
        char: 'タ',
    },
    KeysymChar {
        keysym: 0x04c1,
        char: 'チ',
    },
    KeysymChar {
        keysym: 0x04c2,
        char: 'ツ',
    },
    KeysymChar {
        keysym: 0x04c3,
        char: 'テ',
    },
    KeysymChar {
        keysym: 0x04c4,
        char: 'ト',
    },
    KeysymChar {
        keysym: 0x04c5,
        char: 'ナ',
    },
    KeysymChar {
        keysym: 0x04c6,
        char: 'ニ',
    },
    KeysymChar {
        keysym: 0x04c7,
        char: 'ヌ',
    },
    KeysymChar {
        keysym: 0x04c8,
        char: 'ネ',
    },
    KeysymChar {
        keysym: 0x04c9,
        char: 'ノ',
    },
    KeysymChar {
        keysym: 0x04ca,
        char: 'ハ',
    },
    KeysymChar {
        keysym: 0x04cb,
        char: 'ヒ',
    },
    KeysymChar {
        keysym: 0x04cc,
        char: 'フ',
    },
    KeysymChar {
        keysym: 0x04cd,
        char: 'ヘ',
    },
    KeysymChar {
        keysym: 0x04ce,
        char: 'ホ',
    },
    KeysymChar {
        keysym: 0x04cf,
        char: 'マ',
    },
    KeysymChar {
        keysym: 0x04d0,
        char: 'ミ',
    },
    KeysymChar {
        keysym: 0x04d1,
        char: 'ム',
    },
    KeysymChar {
        keysym: 0x04d2,
        char: 'メ',
    },
    KeysymChar {
        keysym: 0x04d3,
        char: 'モ',
    },
    KeysymChar {
        keysym: 0x04d4,
        char: 'ヤ',
    },
    KeysymChar {
        keysym: 0x04d5,
        char: 'ユ',
    },
    KeysymChar {
        keysym: 0x04d6,
        char: 'ヨ',
    },
    KeysymChar {
        keysym: 0x04d7,
        char: 'ラ',
    },
    KeysymChar {
        keysym: 0x04d8,
        char: 'リ',
    },
    KeysymChar {
        keysym: 0x04d9,
        char: 'ル',
    },
    KeysymChar {
        keysym: 0x04da,
        char: 'レ',
    },
    KeysymChar {
        keysym: 0x04db,
        char: 'ロ',
    },
    KeysymChar {
        keysym: 0x04dc,
        char: 'ワ',
    },
    KeysymChar {
        keysym: 0x04dd,
        char: 'ン',
    },
    KeysymChar {
        keysym: 0x04de,
        char: '゛',
    },
    KeysymChar {
        keysym: 0x04df,
        char: '゜',
    },
    KeysymChar {
        keysym: 0x05ac,
        char: '،',
    },
    KeysymChar {
        keysym: 0x05bb,
        char: '؛',
    },
    KeysymChar {
        keysym: 0x05bf,
        char: '؟',
    },
    KeysymChar {
        keysym: 0x05c1,
        char: 'ء',
    },
    KeysymChar {
        keysym: 0x05c2,
        char: 'آ',
    },
    KeysymChar {
        keysym: 0x05c3,
        char: 'أ',
    },
    KeysymChar {
        keysym: 0x05c4,
        char: 'ؤ',
    },
    KeysymChar {
        keysym: 0x05c5,
        char: 'إ',
    },
    KeysymChar {
        keysym: 0x05c6,
        char: 'ئ',
    },
    KeysymChar {
        keysym: 0x05c7,
        char: 'ا',
    },
    KeysymChar {
        keysym: 0x05c8,
        char: 'ب',
    },
    KeysymChar {
        keysym: 0x05c9,
        char: 'ة',
    },
    KeysymChar {
        keysym: 0x05ca,
        char: 'ت',
    },
    KeysymChar {
        keysym: 0x05cb,
        char: 'ث',
    },
    KeysymChar {
        keysym: 0x05cc,
        char: 'ج',
    },
    KeysymChar {
        keysym: 0x05cd,
        char: 'ح',
    },
    KeysymChar {
        keysym: 0x05ce,
        char: 'خ',
    },
    KeysymChar {
        keysym: 0x05cf,
        char: 'د',
    },
    KeysymChar {
        keysym: 0x05d0,
        char: 'ذ',
    },
    KeysymChar {
        keysym: 0x05d1,
        char: 'ر',
    },
    KeysymChar {
        keysym: 0x05d2,
        char: 'ز',
    },
    KeysymChar {
        keysym: 0x05d3,
        char: 'س',
    },
    KeysymChar {
        keysym: 0x05d4,
        char: 'ش',
    },
    KeysymChar {
        keysym: 0x05d5,
        char: 'ص',
    },
    KeysymChar {
        keysym: 0x05d6,
        char: 'ض',
    },
    KeysymChar {
        keysym: 0x05d7,
        char: 'ط',
    },
    KeysymChar {
        keysym: 0x05d8,
        char: 'ظ',
    },
    KeysymChar {
        keysym: 0x05d9,
        char: 'ع',
    },
    KeysymChar {
        keysym: 0x05da,
        char: 'غ',
    },
    KeysymChar {
        keysym: 0x05e0,
        char: 'ـ',
    },
    KeysymChar {
        keysym: 0x05e1,
        char: 'ف',
    },
    KeysymChar {
        keysym: 0x05e2,
        char: 'ق',
    },
    KeysymChar {
        keysym: 0x05e3,
        char: 'ك',
    },
    KeysymChar {
        keysym: 0x05e4,
        char: 'ل',
    },
    KeysymChar {
        keysym: 0x05e5,
        char: 'م',
    },
    KeysymChar {
        keysym: 0x05e6,
        char: 'ن',
    },
    KeysymChar {
        keysym: 0x05e7,
        char: 'ه',
    },
    KeysymChar {
        keysym: 0x05e8,
        char: 'و',
    },
    KeysymChar {
        keysym: 0x05e9,
        char: 'ى',
    },
    KeysymChar {
        keysym: 0x05ea,
        char: 'ي',
    },
    KeysymChar {
        keysym: 0x05eb,
        char: '\u{64b}',
    },
    KeysymChar {
        keysym: 0x05ec,
        char: '\u{64c}',
    },
    KeysymChar {
        keysym: 0x05ed,
        char: '\u{64d}',
    },
    KeysymChar {
        keysym: 0x05ee,
        char: '\u{64e}',
    },
    KeysymChar {
        keysym: 0x05ef,
        char: '\u{64f}',
    },
    KeysymChar {
        keysym: 0x05f0,
        char: '\u{650}',
    },
    KeysymChar {
        keysym: 0x05f1,
        char: '\u{651}',
    },
    KeysymChar {
        keysym: 0x05f2,
        char: '\u{652}',
    },
    KeysymChar {
        keysym: 0x06a1,
        char: 'ђ',
    },
    KeysymChar {
        keysym: 0x06a2,
        char: 'ѓ',
    },
    KeysymChar {
        keysym: 0x06a3,
        char: 'ё',
    },
    KeysymChar {
        keysym: 0x06a4,
        char: 'є',
    },
    KeysymChar {
        keysym: 0x06a5,
        char: 'ѕ',
    },
    KeysymChar {
        keysym: 0x06a6,
        char: 'і',
    },
    KeysymChar {
        keysym: 0x06a7,
        char: 'ї',
    },
    KeysymChar {
        keysym: 0x06a8,
        char: 'ј',
    },
    KeysymChar {
        keysym: 0x06a9,
        char: 'љ',
    },
    KeysymChar {
        keysym: 0x06aa,
        char: 'њ',
    },
    KeysymChar {
        keysym: 0x06ab,
        char: 'ћ',
    },
    KeysymChar {
        keysym: 0x06ac,
        char: 'ќ',
    },
    KeysymChar {
        keysym: 0x06ad,
        char: 'ґ',
    },
    KeysymChar {
        keysym: 0x06ae,
        char: 'ў',
    },
    KeysymChar {
        keysym: 0x06af,
        char: 'џ',
    },
    KeysymChar {
        keysym: 0x06b0,
        char: '№',
    },
    KeysymChar {
        keysym: 0x06b1,
        char: 'Ђ',
    },
    KeysymChar {
        keysym: 0x06b2,
        char: 'Ѓ',
    },
    KeysymChar {
        keysym: 0x06b3,
        char: 'Ё',
    },
    KeysymChar {
        keysym: 0x06b4,
        char: 'Є',
    },
    KeysymChar {
        keysym: 0x06b5,
        char: 'Ѕ',
    },
    KeysymChar {
        keysym: 0x06b6,
        char: 'І',
    },
    KeysymChar {
        keysym: 0x06b7,
        char: 'Ї',
    },
    KeysymChar {
        keysym: 0x06b8,
        char: 'Ј',
    },
    KeysymChar {
        keysym: 0x06b9,
        char: 'Љ',
    },
    KeysymChar {
        keysym: 0x06ba,
        char: 'Њ',
    },
    KeysymChar {
        keysym: 0x06bb,
        char: 'Ћ',
    },
    KeysymChar {
        keysym: 0x06bc,
        char: 'Ќ',
    },
    KeysymChar {
        keysym: 0x06bd,
        char: 'Ґ',
    },
    KeysymChar {
        keysym: 0x06be,
        char: 'Ў',
    },
    KeysymChar {
        keysym: 0x06bf,
        char: 'Џ',
    },
    KeysymChar {
        keysym: 0x06c0,
        char: 'ю',
    },
    KeysymChar {
        keysym: 0x06c1,
        char: 'а',
    },
    KeysymChar {
        keysym: 0x06c2,
        char: 'б',
    },
    KeysymChar {
        keysym: 0x06c3,
        char: 'ц',
    },
    KeysymChar {
        keysym: 0x06c4,
        char: 'д',
    },
    KeysymChar {
        keysym: 0x06c5,
        char: 'е',
    },
    KeysymChar {
        keysym: 0x06c6,
        char: 'ф',
    },
    KeysymChar {
        keysym: 0x06c7,
        char: 'г',
    },
    KeysymChar {
        keysym: 0x06c8,
        char: 'х',
    },
    KeysymChar {
        keysym: 0x06c9,
        char: 'и',
    },
    KeysymChar {
        keysym: 0x06ca,
        char: 'й',
    },
    KeysymChar {
        keysym: 0x06cb,
        char: 'к',
    },
    KeysymChar {
        keysym: 0x06cc,
        char: 'л',
    },
    KeysymChar {
        keysym: 0x06cd,
        char: 'м',
    },
    KeysymChar {
        keysym: 0x06ce,
        char: 'н',
    },
    KeysymChar {
        keysym: 0x06cf,
        char: 'о',
    },
    KeysymChar {
        keysym: 0x06d0,
        char: 'п',
    },
    KeysymChar {
        keysym: 0x06d1,
        char: 'я',
    },
    KeysymChar {
        keysym: 0x06d2,
        char: 'р',
    },
    KeysymChar {
        keysym: 0x06d3,
        char: 'с',
    },
    KeysymChar {
        keysym: 0x06d4,
        char: 'т',
    },
    KeysymChar {
        keysym: 0x06d5,
        char: 'у',
    },
    KeysymChar {
        keysym: 0x06d6,
        char: 'ж',
    },
    KeysymChar {
        keysym: 0x06d7,
        char: 'в',
    },
    KeysymChar {
        keysym: 0x06d8,
        char: 'ь',
    },
    KeysymChar {
        keysym: 0x06d9,
        char: 'ы',
    },
    KeysymChar {
        keysym: 0x06da,
        char: 'з',
    },
    KeysymChar {
        keysym: 0x06db,
        char: 'ш',
    },
    KeysymChar {
        keysym: 0x06dc,
        char: 'э',
    },
    KeysymChar {
        keysym: 0x06dd,
        char: 'щ',
    },
    KeysymChar {
        keysym: 0x06de,
        char: 'ч',
    },
    KeysymChar {
        keysym: 0x06df,
        char: 'ъ',
    },
    KeysymChar {
        keysym: 0x06e0,
        char: 'Ю',
    },
    KeysymChar {
        keysym: 0x06e1,
        char: 'А',
    },
    KeysymChar {
        keysym: 0x06e2,
        char: 'Б',
    },
    KeysymChar {
        keysym: 0x06e3,
        char: 'Ц',
    },
    KeysymChar {
        keysym: 0x06e4,
        char: 'Д',
    },
    KeysymChar {
        keysym: 0x06e5,
        char: 'Е',
    },
    KeysymChar {
        keysym: 0x06e6,
        char: 'Ф',
    },
    KeysymChar {
        keysym: 0x06e7,
        char: 'Г',
    },
    KeysymChar {
        keysym: 0x06e8,
        char: 'Х',
    },
    KeysymChar {
        keysym: 0x06e9,
        char: 'И',
    },
    KeysymChar {
        keysym: 0x06ea,
        char: 'Й',
    },
    KeysymChar {
        keysym: 0x06eb,
        char: 'К',
    },
    KeysymChar {
        keysym: 0x06ec,
        char: 'Л',
    },
    KeysymChar {
        keysym: 0x06ed,
        char: 'М',
    },
    KeysymChar {
        keysym: 0x06ee,
        char: 'Н',
    },
    KeysymChar {
        keysym: 0x06ef,
        char: 'О',
    },
    KeysymChar {
        keysym: 0x06f0,
        char: 'П',
    },
    KeysymChar {
        keysym: 0x06f1,
        char: 'Я',
    },
    KeysymChar {
        keysym: 0x06f2,
        char: 'Р',
    },
    KeysymChar {
        keysym: 0x06f3,
        char: 'С',
    },
    KeysymChar {
        keysym: 0x06f4,
        char: 'Т',
    },
    KeysymChar {
        keysym: 0x06f5,
        char: 'У',
    },
    KeysymChar {
        keysym: 0x06f6,
        char: 'Ж',
    },
    KeysymChar {
        keysym: 0x06f7,
        char: 'В',
    },
    KeysymChar {
        keysym: 0x06f8,
        char: 'Ь',
    },
    KeysymChar {
        keysym: 0x06f9,
        char: 'Ы',
    },
    KeysymChar {
        keysym: 0x06fa,
        char: 'З',
    },
    KeysymChar {
        keysym: 0x06fb,
        char: 'Ш',
    },
    KeysymChar {
        keysym: 0x06fc,
        char: 'Э',
    },
    KeysymChar {
        keysym: 0x06fd,
        char: 'Щ',
    },
    KeysymChar {
        keysym: 0x06fe,
        char: 'Ч',
    },
    KeysymChar {
        keysym: 0x06ff,
        char: 'Ъ',
    },
    KeysymChar {
        keysym: 0x07a1,
        char: 'Ά',
    },
    KeysymChar {
        keysym: 0x07a2,
        char: 'Έ',
    },
    KeysymChar {
        keysym: 0x07a3,
        char: 'Ή',
    },
    KeysymChar {
        keysym: 0x07a4,
        char: 'Ί',
    },
    KeysymChar {
        keysym: 0x07a5,
        char: 'Ϊ',
    },
    KeysymChar {
        keysym: 0x07a7,
        char: 'Ό',
    },
    KeysymChar {
        keysym: 0x07a8,
        char: 'Ύ',
    },
    KeysymChar {
        keysym: 0x07a9,
        char: 'Ϋ',
    },
    KeysymChar {
        keysym: 0x07ab,
        char: 'Ώ',
    },
    KeysymChar {
        keysym: 0x07ae,
        char: '΅',
    },
    KeysymChar {
        keysym: 0x07af,
        char: '―',
    },
    KeysymChar {
        keysym: 0x07b1,
        char: 'ά',
    },
    KeysymChar {
        keysym: 0x07b2,
        char: 'έ',
    },
    KeysymChar {
        keysym: 0x07b3,
        char: 'ή',
    },
    KeysymChar {
        keysym: 0x07b4,
        char: 'ί',
    },
    KeysymChar {
        keysym: 0x07b5,
        char: 'ϊ',
    },
    KeysymChar {
        keysym: 0x07b6,
        char: 'ΐ',
    },
    KeysymChar {
        keysym: 0x07b7,
        char: 'ό',
    },
    KeysymChar {
        keysym: 0x07b8,
        char: 'ύ',
    },
    KeysymChar {
        keysym: 0x07b9,
        char: 'ϋ',
    },
    KeysymChar {
        keysym: 0x07ba,
        char: 'ΰ',
    },
    KeysymChar {
        keysym: 0x07bb,
        char: 'ώ',
    },
    KeysymChar {
        keysym: 0x07c1,
        char: 'Α',
    },
    KeysymChar {
        keysym: 0x07c2,
        char: 'Β',
    },
    KeysymChar {
        keysym: 0x07c3,
        char: 'Γ',
    },
    KeysymChar {
        keysym: 0x07c4,
        char: 'Δ',
    },
    KeysymChar {
        keysym: 0x07c5,
        char: 'Ε',
    },
    KeysymChar {
        keysym: 0x07c6,
        char: 'Ζ',
    },
    KeysymChar {
        keysym: 0x07c7,
        char: 'Η',
    },
    KeysymChar {
        keysym: 0x07c8,
        char: 'Θ',
    },
    KeysymChar {
        keysym: 0x07c9,
        char: 'Ι',
    },
    KeysymChar {
        keysym: 0x07ca,
        char: 'Κ',
    },
    KeysymChar {
        keysym: 0x07cb,
        char: 'Λ',
    },
    KeysymChar {
        keysym: 0x07cc,
        char: 'Μ',
    },
    KeysymChar {
        keysym: 0x07cd,
        char: 'Ν',
    },
    KeysymChar {
        keysym: 0x07ce,
        char: 'Ξ',
    },
    KeysymChar {
        keysym: 0x07cf,
        char: 'Ο',
    },
    KeysymChar {
        keysym: 0x07d0,
        char: 'Π',
    },
    KeysymChar {
        keysym: 0x07d1,
        char: 'Ρ',
    },
    KeysymChar {
        keysym: 0x07d2,
        char: 'Σ',
    },
    KeysymChar {
        keysym: 0x07d4,
        char: 'Τ',
    },
    KeysymChar {
        keysym: 0x07d5,
        char: 'Υ',
    },
    KeysymChar {
        keysym: 0x07d6,
        char: 'Φ',
    },
    KeysymChar {
        keysym: 0x07d7,
        char: 'Χ',
    },
    KeysymChar {
        keysym: 0x07d8,
        char: 'Ψ',
    },
    KeysymChar {
        keysym: 0x07d9,
        char: 'Ω',
    },
    KeysymChar {
        keysym: 0x07e1,
        char: 'α',
    },
    KeysymChar {
        keysym: 0x07e2,
        char: 'β',
    },
    KeysymChar {
        keysym: 0x07e3,
        char: 'γ',
    },
    KeysymChar {
        keysym: 0x07e4,
        char: 'δ',
    },
    KeysymChar {
        keysym: 0x07e5,
        char: 'ε',
    },
    KeysymChar {
        keysym: 0x07e6,
        char: 'ζ',
    },
    KeysymChar {
        keysym: 0x07e7,
        char: 'η',
    },
    KeysymChar {
        keysym: 0x07e8,
        char: 'θ',
    },
    KeysymChar {
        keysym: 0x07e9,
        char: 'ι',
    },
    KeysymChar {
        keysym: 0x07ea,
        char: 'κ',
    },
    KeysymChar {
        keysym: 0x07eb,
        char: 'λ',
    },
    KeysymChar {
        keysym: 0x07ec,
        char: 'μ',
    },
    KeysymChar {
        keysym: 0x07ed,
        char: 'ν',
    },
    KeysymChar {
        keysym: 0x07ee,
        char: 'ξ',
    },
    KeysymChar {
        keysym: 0x07ef,
        char: 'ο',
    },
    KeysymChar {
        keysym: 0x07f0,
        char: 'π',
    },
    KeysymChar {
        keysym: 0x07f1,
        char: 'ρ',
    },
    KeysymChar {
        keysym: 0x07f2,
        char: 'σ',
    },
    KeysymChar {
        keysym: 0x07f3,
        char: 'ς',
    },
    KeysymChar {
        keysym: 0x07f4,
        char: 'τ',
    },
    KeysymChar {
        keysym: 0x07f5,
        char: 'υ',
    },
    KeysymChar {
        keysym: 0x07f6,
        char: 'φ',
    },
    KeysymChar {
        keysym: 0x07f7,
        char: 'χ',
    },
    KeysymChar {
        keysym: 0x07f8,
        char: 'ψ',
    },
    KeysymChar {
        keysym: 0x07f9,
        char: 'ω',
    },
    KeysymChar {
        keysym: 0x08a1,
        char: '⎷',
    },
    KeysymChar {
        keysym: 0x08a2,
        char: '┌',
    },
    KeysymChar {
        keysym: 0x08a3,
        char: '─',
    },
    KeysymChar {
        keysym: 0x08a4,
        char: '⌠',
    },
    KeysymChar {
        keysym: 0x08a5,
        char: '⌡',
    },
    KeysymChar {
        keysym: 0x08a6,
        char: '│',
    },
    KeysymChar {
        keysym: 0x08a7,
        char: '⎡',
    },
    KeysymChar {
        keysym: 0x08a8,
        char: '⎣',
    },
    KeysymChar {
        keysym: 0x08a9,
        char: '⎤',
    },
    KeysymChar {
        keysym: 0x08aa,
        char: '⎦',
    },
    KeysymChar {
        keysym: 0x08ab,
        char: '⎛',
    },
    KeysymChar {
        keysym: 0x08ac,
        char: '⎝',
    },
    KeysymChar {
        keysym: 0x08ad,
        char: '⎞',
    },
    KeysymChar {
        keysym: 0x08ae,
        char: '⎠',
    },
    KeysymChar {
        keysym: 0x08af,
        char: '⎨',
    },
    KeysymChar {
        keysym: 0x08b0,
        char: '⎬',
    },
    KeysymChar {
        keysym: 0x08bc,
        char: '≤',
    },
    KeysymChar {
        keysym: 0x08bd,
        char: '≠',
    },
    KeysymChar {
        keysym: 0x08be,
        char: '≥',
    },
    KeysymChar {
        keysym: 0x08bf,
        char: '∫',
    },
    KeysymChar {
        keysym: 0x08c0,
        char: '∴',
    },
    KeysymChar {
        keysym: 0x08c1,
        char: '∝',
    },
    KeysymChar {
        keysym: 0x08c2,
        char: '∞',
    },
    KeysymChar {
        keysym: 0x08c5,
        char: '∇',
    },
    KeysymChar {
        keysym: 0x08c8,
        char: '∼',
    },
    KeysymChar {
        keysym: 0x08c9,
        char: '≃',
    },
    KeysymChar {
        keysym: 0x08cd,
        char: '⇔',
    },
    KeysymChar {
        keysym: 0x08ce,
        char: '⇒',
    },
    KeysymChar {
        keysym: 0x08cf,
        char: '≡',
    },
    KeysymChar {
        keysym: 0x08d6,
        char: '√',
    },
    KeysymChar {
        keysym: 0x08da,
        char: '⊂',
    },
    KeysymChar {
        keysym: 0x08db,
        char: '⊃',
    },
    KeysymChar {
        keysym: 0x08dc,
        char: '∩',
    },
    KeysymChar {
        keysym: 0x08dd,
        char: '∪',
    },
    KeysymChar {
        keysym: 0x08de,
        char: '∧',
    },
    KeysymChar {
        keysym: 0x08df,
        char: '∨',
    },
    KeysymChar {
        keysym: 0x08ef,
        char: '∂',
    },
    KeysymChar {
        keysym: 0x08f6,
        char: 'ƒ',
    },
    KeysymChar {
        keysym: 0x08fb,
        char: '←',
    },
    KeysymChar {
        keysym: 0x08fc,
        char: '↑',
    },
    KeysymChar {
        keysym: 0x08fd,
        char: '→',
    },
    KeysymChar {
        keysym: 0x08fe,
        char: '↓',
    },
    KeysymChar {
        keysym: 0x09e0,
        char: '◆',
    },
    KeysymChar {
        keysym: 0x09e1,
        char: '▒',
    },
    KeysymChar {
        keysym: 0x09e2,
        char: '␉',
    },
    KeysymChar {
        keysym: 0x09e3,
        char: '␌',
    },
    KeysymChar {
        keysym: 0x09e4,
        char: '␍',
    },
    KeysymChar {
        keysym: 0x09e5,
        char: '␊',
    },
    KeysymChar {
        keysym: 0x09e8,
        char: '␤',
    },
    KeysymChar {
        keysym: 0x09e9,
        char: '␋',
    },
    KeysymChar {
        keysym: 0x09ea,
        char: '┘',
    },
    KeysymChar {
        keysym: 0x09eb,
        char: '┐',
    },
    KeysymChar {
        keysym: 0x09ec,
        char: '┌',
    },
    KeysymChar {
        keysym: 0x09ed,
        char: '└',
    },
    KeysymChar {
        keysym: 0x09ee,
        char: '┼',
    },
    KeysymChar {
        keysym: 0x09ef,
        char: '⎺',
    },
    KeysymChar {
        keysym: 0x09f0,
        char: '⎻',
    },
    KeysymChar {
        keysym: 0x09f1,
        char: '─',
    },
    KeysymChar {
        keysym: 0x09f2,
        char: '⎼',
    },
    KeysymChar {
        keysym: 0x09f3,
        char: '⎽',
    },
    KeysymChar {
        keysym: 0x09f4,
        char: '├',
    },
    KeysymChar {
        keysym: 0x09f5,
        char: '┤',
    },
    KeysymChar {
        keysym: 0x09f6,
        char: '┴',
    },
    KeysymChar {
        keysym: 0x09f7,
        char: '┬',
    },
    KeysymChar {
        keysym: 0x09f8,
        char: '│',
    },
    KeysymChar {
        keysym: 0x0aa1,
        char: '\u{2003}',
    },
    KeysymChar {
        keysym: 0x0aa2,
        char: '\u{2002}',
    },
    KeysymChar {
        keysym: 0x0aa3,
        char: '\u{2004}',
    },
    KeysymChar {
        keysym: 0x0aa4,
        char: '\u{2005}',
    },
    KeysymChar {
        keysym: 0x0aa5,
        char: '\u{2007}',
    },
    KeysymChar {
        keysym: 0x0aa6,
        char: '\u{2008}',
    },
    KeysymChar {
        keysym: 0x0aa7,
        char: '\u{2009}',
    },
    KeysymChar {
        keysym: 0x0aa8,
        char: '\u{200a}',
    },
    KeysymChar {
        keysym: 0x0aa9,
        char: '—',
    },
    KeysymChar {
        keysym: 0x0aaa,
        char: '–',
    },
    KeysymChar {
        keysym: 0x0aac,
        char: '␣',
    },
    KeysymChar {
        keysym: 0x0aae,
        char: '…',
    },
    KeysymChar {
        keysym: 0x0aaf,
        char: '‥',
    },
    KeysymChar {
        keysym: 0x0ab0,
        char: '⅓',
    },
    KeysymChar {
        keysym: 0x0ab1,
        char: '⅔',
    },
    KeysymChar {
        keysym: 0x0ab2,
        char: '⅕',
    },
    KeysymChar {
        keysym: 0x0ab3,
        char: '⅖',
    },
    KeysymChar {
        keysym: 0x0ab4,
        char: '⅗',
    },
    KeysymChar {
        keysym: 0x0ab5,
        char: '⅘',
    },
    KeysymChar {
        keysym: 0x0ab6,
        char: '⅙',
    },
    KeysymChar {
        keysym: 0x0ab7,
        char: '⅚',
    },
    KeysymChar {
        keysym: 0x0ab8,
        char: '℅',
    },
    KeysymChar {
        keysym: 0x0abb,
        char: '‒',
    },
    KeysymChar {
        keysym: 0x0abc,
        char: '⟨',
    },
    KeysymChar {
        keysym: 0x0abd,
        char: '.',
    },
    KeysymChar {
        keysym: 0x0abe,
        char: '⟩',
    },
    KeysymChar {
        keysym: 0x0ac3,
        char: '⅛',
    },
    KeysymChar {
        keysym: 0x0ac4,
        char: '⅜',
    },
    KeysymChar {
        keysym: 0x0ac5,
        char: '⅝',
    },
    KeysymChar {
        keysym: 0x0ac6,
        char: '⅞',
    },
    KeysymChar {
        keysym: 0x0ac9,
        char: '™',
    },
    KeysymChar {
        keysym: 0x0aca,
        char: '☓',
    },
    KeysymChar {
        keysym: 0x0acc,
        char: '◁',
    },
    KeysymChar {
        keysym: 0x0acd,
        char: '▷',
    },
    KeysymChar {
        keysym: 0x0ace,
        char: '○',
    },
    KeysymChar {
        keysym: 0x0acf,
        char: '▯',
    },
    KeysymChar {
        keysym: 0x0ad0,
        char: '‘',
    },
    KeysymChar {
        keysym: 0x0ad1,
        char: '’',
    },
    KeysymChar {
        keysym: 0x0ad2,
        char: '“',
    },
    KeysymChar {
        keysym: 0x0ad3,
        char: '”',
    },
    KeysymChar {
        keysym: 0x0ad4,
        char: '℞',
    },
    KeysymChar {
        keysym: 0x0ad5,
        char: '‰',
    },
    KeysymChar {
        keysym: 0x0ad6,
        char: '′',
    },
    KeysymChar {
        keysym: 0x0ad7,
        char: '″',
    },
    KeysymChar {
        keysym: 0x0ad9,
        char: '✝',
    },
    KeysymChar {
        keysym: 0x0adb,
        char: '▬',
    },
    KeysymChar {
        keysym: 0x0adc,
        char: '◀',
    },
    KeysymChar {
        keysym: 0x0add,
        char: '▶',
    },
    KeysymChar {
        keysym: 0x0ade,
        char: '●',
    },
    KeysymChar {
        keysym: 0x0adf,
        char: '▮',
    },
    KeysymChar {
        keysym: 0x0ae0,
        char: '◦',
    },
    KeysymChar {
        keysym: 0x0ae1,
        char: '▫',
    },
    KeysymChar {
        keysym: 0x0ae2,
        char: '▭',
    },
    KeysymChar {
        keysym: 0x0ae3,
        char: '△',
    },
    KeysymChar {
        keysym: 0x0ae4,
        char: '▽',
    },
    KeysymChar {
        keysym: 0x0ae5,
        char: '☆',
    },
    KeysymChar {
        keysym: 0x0ae6,
        char: '•',
    },
    KeysymChar {
        keysym: 0x0ae7,
        char: '▪',
    },
    KeysymChar {
        keysym: 0x0ae8,
        char: '▲',
    },
    KeysymChar {
        keysym: 0x0ae9,
        char: '▼',
    },
    KeysymChar {
        keysym: 0x0aea,
        char: '☜',
    },
    KeysymChar {
        keysym: 0x0aeb,
        char: '☞',
    },
    KeysymChar {
        keysym: 0x0aec,
        char: '♣',
    },
    KeysymChar {
        keysym: 0x0aed,
        char: '♦',
    },
    KeysymChar {
        keysym: 0x0aee,
        char: '♥',
    },
    KeysymChar {
        keysym: 0x0af0,
        char: '✠',
    },
    KeysymChar {
        keysym: 0x0af1,
        char: '†',
    },
    KeysymChar {
        keysym: 0x0af2,
        char: '‡',
    },
    KeysymChar {
        keysym: 0x0af3,
        char: '✓',
    },
    KeysymChar {
        keysym: 0x0af4,
        char: '✗',
    },
    KeysymChar {
        keysym: 0x0af5,
        char: '♯',
    },
    KeysymChar {
        keysym: 0x0af6,
        char: '♭',
    },
    KeysymChar {
        keysym: 0x0af7,
        char: '♂',
    },
    KeysymChar {
        keysym: 0x0af8,
        char: '♀',
    },
    KeysymChar {
        keysym: 0x0af9,
        char: '☎',
    },
    KeysymChar {
        keysym: 0x0afa,
        char: '⌕',
    },
    KeysymChar {
        keysym: 0x0afb,
        char: '℗',
    },
    KeysymChar {
        keysym: 0x0afc,
        char: '‸',
    },
    KeysymChar {
        keysym: 0x0afd,
        char: '‚',
    },
    KeysymChar {
        keysym: 0x0afe,
        char: '„',
    },
    KeysymChar {
        keysym: 0x0ba3,
        char: '<',
    },
    KeysymChar {
        keysym: 0x0ba6,
        char: '>',
    },
    KeysymChar {
        keysym: 0x0ba8,
        char: '∨',
    },
    KeysymChar {
        keysym: 0x0ba9,
        char: '∧',
    },
    KeysymChar {
        keysym: 0x0bc0,
        char: '¯',
    },
    KeysymChar {
        keysym: 0x0bc2,
        char: '⊤',
    },
    KeysymChar {
        keysym: 0x0bc3,
        char: '∩',
    },
    KeysymChar {
        keysym: 0x0bc4,
        char: '⌊',
    },
    KeysymChar {
        keysym: 0x0bc6,
        char: '_',
    },
    KeysymChar {
        keysym: 0x0bca,
        char: '∘',
    },
    KeysymChar {
        keysym: 0x0bcc,
        char: '⎕',
    },
    KeysymChar {
        keysym: 0x0bce,
        char: '⊥',
    },
    KeysymChar {
        keysym: 0x0bcf,
        char: '○',
    },
    KeysymChar {
        keysym: 0x0bd3,
        char: '⌈',
    },
    KeysymChar {
        keysym: 0x0bd6,
        char: '∪',
    },
    KeysymChar {
        keysym: 0x0bd8,
        char: '⊃',
    },
    KeysymChar {
        keysym: 0x0bda,
        char: '⊂',
    },
    KeysymChar {
        keysym: 0x0bdc,
        char: '⊣',
    },
    KeysymChar {
        keysym: 0x0bfc,
        char: '⊢',
    },
    KeysymChar {
        keysym: 0x0cdf,
        char: '‗',
    },
    KeysymChar {
        keysym: 0x0ce0,
        char: 'א',
    },
    KeysymChar {
        keysym: 0x0ce1,
        char: 'ב',
    },
    KeysymChar {
        keysym: 0x0ce2,
        char: 'ג',
    },
    KeysymChar {
        keysym: 0x0ce3,
        char: 'ד',
    },
    KeysymChar {
        keysym: 0x0ce4,
        char: 'ה',
    },
    KeysymChar {
        keysym: 0x0ce5,
        char: 'ו',
    },
    KeysymChar {
        keysym: 0x0ce6,
        char: 'ז',
    },
    KeysymChar {
        keysym: 0x0ce7,
        char: 'ח',
    },
    KeysymChar {
        keysym: 0x0ce8,
        char: 'ט',
    },
    KeysymChar {
        keysym: 0x0ce9,
        char: 'י',
    },
    KeysymChar {
        keysym: 0x0cea,
        char: 'ך',
    },
    KeysymChar {
        keysym: 0x0ceb,
        char: 'כ',
    },
    KeysymChar {
        keysym: 0x0cec,
        char: 'ל',
    },
    KeysymChar {
        keysym: 0x0ced,
        char: 'ם',
    },
    KeysymChar {
        keysym: 0x0cee,
        char: 'מ',
    },
    KeysymChar {
        keysym: 0x0cef,
        char: 'ן',
    },
    KeysymChar {
        keysym: 0x0cf0,
        char: 'נ',
    },
    KeysymChar {
        keysym: 0x0cf1,
        char: 'ס',
    },
    KeysymChar {
        keysym: 0x0cf2,
        char: 'ע',
    },
    KeysymChar {
        keysym: 0x0cf3,
        char: 'ף',
    },
    KeysymChar {
        keysym: 0x0cf4,
        char: 'פ',
    },
    KeysymChar {
        keysym: 0x0cf5,
        char: 'ץ',
    },
    KeysymChar {
        keysym: 0x0cf6,
        char: 'צ',
    },
    KeysymChar {
        keysym: 0x0cf7,
        char: 'ק',
    },
    KeysymChar {
        keysym: 0x0cf8,
        char: 'ר',
    },
    KeysymChar {
        keysym: 0x0cf9,
        char: 'ש',
    },
    KeysymChar {
        keysym: 0x0cfa,
        char: 'ת',
    },
    KeysymChar {
        keysym: 0x0da1,
        char: 'ก',
    },
    KeysymChar {
        keysym: 0x0da2,
        char: 'ข',
    },
    KeysymChar {
        keysym: 0x0da3,
        char: 'ฃ',
    },
    KeysymChar {
        keysym: 0x0da4,
        char: 'ค',
    },
    KeysymChar {
        keysym: 0x0da5,
        char: 'ฅ',
    },
    KeysymChar {
        keysym: 0x0da6,
        char: 'ฆ',
    },
    KeysymChar {
        keysym: 0x0da7,
        char: 'ง',
    },
    KeysymChar {
        keysym: 0x0da8,
        char: 'จ',
    },
    KeysymChar {
        keysym: 0x0da9,
        char: 'ฉ',
    },
    KeysymChar {
        keysym: 0x0daa,
        char: 'ช',
    },
    KeysymChar {
        keysym: 0x0dab,
        char: 'ซ',
    },
    KeysymChar {
        keysym: 0x0dac,
        char: 'ฌ',
    },
    KeysymChar {
        keysym: 0x0dad,
        char: 'ญ',
    },
    KeysymChar {
        keysym: 0x0dae,
        char: 'ฎ',
    },
    KeysymChar {
        keysym: 0x0daf,
        char: 'ฏ',
    },
    KeysymChar {
        keysym: 0x0db0,
        char: 'ฐ',
    },
    KeysymChar {
        keysym: 0x0db1,
        char: 'ฑ',
    },
    KeysymChar {
        keysym: 0x0db2,
        char: 'ฒ',
    },
    KeysymChar {
        keysym: 0x0db3,
        char: 'ณ',
    },
    KeysymChar {
        keysym: 0x0db4,
        char: 'ด',
    },
    KeysymChar {
        keysym: 0x0db5,
        char: 'ต',
    },
    KeysymChar {
        keysym: 0x0db6,
        char: 'ถ',
    },
    KeysymChar {
        keysym: 0x0db7,
        char: 'ท',
    },
    KeysymChar {
        keysym: 0x0db8,
        char: 'ธ',
    },
    KeysymChar {
        keysym: 0x0db9,
        char: 'น',
    },
    KeysymChar {
        keysym: 0x0dba,
        char: 'บ',
    },
    KeysymChar {
        keysym: 0x0dbb,
        char: 'ป',
    },
    KeysymChar {
        keysym: 0x0dbc,
        char: 'ผ',
    },
    KeysymChar {
        keysym: 0x0dbd,
        char: 'ฝ',
    },
    KeysymChar {
        keysym: 0x0dbe,
        char: 'พ',
    },
    KeysymChar {
        keysym: 0x0dbf,
        char: 'ฟ',
    },
    KeysymChar {
        keysym: 0x0dc0,
        char: 'ภ',
    },
    KeysymChar {
        keysym: 0x0dc1,
        char: 'ม',
    },
    KeysymChar {
        keysym: 0x0dc2,
        char: 'ย',
    },
    KeysymChar {
        keysym: 0x0dc3,
        char: 'ร',
    },
    KeysymChar {
        keysym: 0x0dc4,
        char: 'ฤ',
    },
    KeysymChar {
        keysym: 0x0dc5,
        char: 'ล',
    },
    KeysymChar {
        keysym: 0x0dc6,
        char: 'ฦ',
    },
    KeysymChar {
        keysym: 0x0dc7,
        char: 'ว',
    },
    KeysymChar {
        keysym: 0x0dc8,
        char: 'ศ',
    },
    KeysymChar {
        keysym: 0x0dc9,
        char: 'ษ',
    },
    KeysymChar {
        keysym: 0x0dca,
        char: 'ส',
    },
    KeysymChar {
        keysym: 0x0dcb,
        char: 'ห',
    },
    KeysymChar {
        keysym: 0x0dcc,
        char: 'ฬ',
    },
    KeysymChar {
        keysym: 0x0dcd,
        char: 'อ',
    },
    KeysymChar {
        keysym: 0x0dce,
        char: 'ฮ',
    },
    KeysymChar {
        keysym: 0x0dcf,
        char: 'ฯ',
    },
    KeysymChar {
        keysym: 0x0dd0,
        char: 'ะ',
    },
    KeysymChar {
        keysym: 0x0dd1,
        char: '\u{e31}',
    },
    KeysymChar {
        keysym: 0x0dd2,
        char: 'า',
    },
    KeysymChar {
        keysym: 0x0dd3,
        char: 'ำ',
    },
    KeysymChar {
        keysym: 0x0dd4,
        char: '\u{e34}',
    },
    KeysymChar {
        keysym: 0x0dd5,
        char: '\u{e35}',
    },
    KeysymChar {
        keysym: 0x0dd6,
        char: '\u{e36}',
    },
    KeysymChar {
        keysym: 0x0dd7,
        char: '\u{e37}',
    },
    KeysymChar {
        keysym: 0x0dd8,
        char: '\u{e38}',
    },
    KeysymChar {
        keysym: 0x0dd9,
        char: '\u{e39}',
    },
    KeysymChar {
        keysym: 0x0dda,
        char: '\u{e3a}',
    },
    KeysymChar {
        keysym: 0x0dde,
        char: '\u{e3e}',
    },
    KeysymChar {
        keysym: 0x0ddf,
        char: '฿',
    },
    KeysymChar {
        keysym: 0x0de0,
        char: 'เ',
    },
    KeysymChar {
        keysym: 0x0de1,
        char: 'แ',
    },
    KeysymChar {
        keysym: 0x0de2,
        char: 'โ',
    },
    KeysymChar {
        keysym: 0x0de3,
        char: 'ใ',
    },
    KeysymChar {
        keysym: 0x0de4,
        char: 'ไ',
    },
    KeysymChar {
        keysym: 0x0de5,
        char: 'ๅ',
    },
    KeysymChar {
        keysym: 0x0de6,
        char: 'ๆ',
    },
    KeysymChar {
        keysym: 0x0de7,
        char: '\u{e47}',
    },
    KeysymChar {
        keysym: 0x0de8,
        char: '\u{e48}',
    },
    KeysymChar {
        keysym: 0x0de9,
        char: '\u{e49}',
    },
    KeysymChar {
        keysym: 0x0dea,
        char: '\u{e4a}',
    },
    KeysymChar {
        keysym: 0x0deb,
        char: '\u{e4b}',
    },
    KeysymChar {
        keysym: 0x0dec,
        char: '\u{e4c}',
    },
    KeysymChar {
        keysym: 0x0ded,
        char: '\u{e4d}',
    },
    KeysymChar {
        keysym: 0x0df0,
        char: '๐',
    },
    KeysymChar {
        keysym: 0x0df1,
        char: '๑',
    },
    KeysymChar {
        keysym: 0x0df2,
        char: '๒',
    },
    KeysymChar {
        keysym: 0x0df3,
        char: '๓',
    },
    KeysymChar {
        keysym: 0x0df4,
        char: '๔',
    },
    KeysymChar {
        keysym: 0x0df5,
        char: '๕',
    },
    KeysymChar {
        keysym: 0x0df6,
        char: '๖',
    },
    KeysymChar {
        keysym: 0x0df7,
        char: '๗',
    },
    KeysymChar {
        keysym: 0x0df8,
        char: '๘',
    },
    KeysymChar {
        keysym: 0x0df9,
        char: '๙',
    },
    KeysymChar {
        keysym: 0x0ea1,
        char: 'ㄱ',
    },
    KeysymChar {
        keysym: 0x0ea2,
        char: 'ㄲ',
    },
    KeysymChar {
        keysym: 0x0ea3,
        char: 'ㄳ',
    },
    KeysymChar {
        keysym: 0x0ea4,
        char: 'ㄴ',
    },
    KeysymChar {
        keysym: 0x0ea5,
        char: 'ㄵ',
    },
    KeysymChar {
        keysym: 0x0ea6,
        char: 'ㄶ',
    },
    KeysymChar {
        keysym: 0x0ea7,
        char: 'ㄷ',
    },
    KeysymChar {
        keysym: 0x0ea8,
        char: 'ㄸ',
    },
    KeysymChar {
        keysym: 0x0ea9,
        char: 'ㄹ',
    },
    KeysymChar {
        keysym: 0x0eaa,
        char: 'ㄺ',
    },
    KeysymChar {
        keysym: 0x0eab,
        char: 'ㄻ',
    },
    KeysymChar {
        keysym: 0x0eac,
        char: 'ㄼ',
    },
    KeysymChar {
        keysym: 0x0ead,
        char: 'ㄽ',
    },
    KeysymChar {
        keysym: 0x0eae,
        char: 'ㄾ',
    },
    KeysymChar {
        keysym: 0x0eaf,
        char: 'ㄿ',
    },
    KeysymChar {
        keysym: 0x0eb0,
        char: 'ㅀ',
    },
    KeysymChar {
        keysym: 0x0eb1,
        char: 'ㅁ',
    },
    KeysymChar {
        keysym: 0x0eb2,
        char: 'ㅂ',
    },
    KeysymChar {
        keysym: 0x0eb3,
        char: 'ㅃ',
    },
    KeysymChar {
        keysym: 0x0eb4,
        char: 'ㅄ',
    },
    KeysymChar {
        keysym: 0x0eb5,
        char: 'ㅅ',
    },
    KeysymChar {
        keysym: 0x0eb6,
        char: 'ㅆ',
    },
    KeysymChar {
        keysym: 0x0eb7,
        char: 'ㅇ',
    },
    KeysymChar {
        keysym: 0x0eb8,
        char: 'ㅈ',
    },
    KeysymChar {
        keysym: 0x0eb9,
        char: 'ㅉ',
    },
    KeysymChar {
        keysym: 0x0eba,
        char: 'ㅊ',
    },
    KeysymChar {
        keysym: 0x0ebb,
        char: 'ㅋ',
    },
    KeysymChar {
        keysym: 0x0ebc,
        char: 'ㅌ',
    },
    KeysymChar {
        keysym: 0x0ebd,
        char: 'ㅍ',
    },
    KeysymChar {
        keysym: 0x0ebe,
        char: 'ㅎ',
    },
    KeysymChar {
        keysym: 0x0ebf,
        char: 'ㅏ',
    },
    KeysymChar {
        keysym: 0x0ec0,
        char: 'ㅐ',
    },
    KeysymChar {
        keysym: 0x0ec1,
        char: 'ㅑ',
    },
    KeysymChar {
        keysym: 0x0ec2,
        char: 'ㅒ',
    },
    KeysymChar {
        keysym: 0x0ec3,
        char: 'ㅓ',
    },
    KeysymChar {
        keysym: 0x0ec4,
        char: 'ㅔ',
    },
    KeysymChar {
        keysym: 0x0ec5,
        char: 'ㅕ',
    },
    KeysymChar {
        keysym: 0x0ec6,
        char: 'ㅖ',
    },
    KeysymChar {
        keysym: 0x0ec7,
        char: 'ㅗ',
    },
    KeysymChar {
        keysym: 0x0ec8,
        char: 'ㅘ',
    },
    KeysymChar {
        keysym: 0x0ec9,
        char: 'ㅙ',
    },
    KeysymChar {
        keysym: 0x0eca,
        char: 'ㅚ',
    },
    KeysymChar {
        keysym: 0x0ecb,
        char: 'ㅛ',
    },
    KeysymChar {
        keysym: 0x0ecc,
        char: 'ㅜ',
    },
    KeysymChar {
        keysym: 0x0ecd,
        char: 'ㅝ',
    },
    KeysymChar {
        keysym: 0x0ece,
        char: 'ㅞ',
    },
    KeysymChar {
        keysym: 0x0ecf,
        char: 'ㅟ',
    },
    KeysymChar {
        keysym: 0x0ed0,
        char: 'ㅠ',
    },
    KeysymChar {
        keysym: 0x0ed1,
        char: 'ㅡ',
    },
    KeysymChar {
        keysym: 0x0ed2,
        char: 'ㅢ',
    },
    KeysymChar {
        keysym: 0x0ed3,
        char: 'ㅣ',
    },
    KeysymChar {
        keysym: 0x0ed4,
        char: 'ᆨ',
    },
    KeysymChar {
        keysym: 0x0ed5,
        char: 'ᆩ',
    },
    KeysymChar {
        keysym: 0x0ed6,
        char: 'ᆪ',
    },
    KeysymChar {
        keysym: 0x0ed7,
        char: 'ᆫ',
    },
    KeysymChar {
        keysym: 0x0ed8,
        char: 'ᆬ',
    },
    KeysymChar {
        keysym: 0x0ed9,
        char: 'ᆭ',
    },
    KeysymChar {
        keysym: 0x0eda,
        char: 'ᆮ',
    },
    KeysymChar {
        keysym: 0x0edb,
        char: 'ᆯ',
    },
    KeysymChar {
        keysym: 0x0edc,
        char: 'ᆰ',
    },
    KeysymChar {
        keysym: 0x0edd,
        char: 'ᆱ',
    },
    KeysymChar {
        keysym: 0x0ede,
        char: 'ᆲ',
    },
    KeysymChar {
        keysym: 0x0edf,
        char: 'ᆳ',
    },
    KeysymChar {
        keysym: 0x0ee0,
        char: 'ᆴ',
    },
    KeysymChar {
        keysym: 0x0ee1,
        char: 'ᆵ',
    },
    KeysymChar {
        keysym: 0x0ee2,
        char: 'ᆶ',
    },
    KeysymChar {
        keysym: 0x0ee3,
        char: 'ᆷ',
    },
    KeysymChar {
        keysym: 0x0ee4,
        char: 'ᆸ',
    },
    KeysymChar {
        keysym: 0x0ee5,
        char: 'ᆹ',
    },
    KeysymChar {
        keysym: 0x0ee6,
        char: 'ᆺ',
    },
    KeysymChar {
        keysym: 0x0ee7,
        char: 'ᆻ',
    },
    KeysymChar {
        keysym: 0x0ee8,
        char: 'ᆼ',
    },
    KeysymChar {
        keysym: 0x0ee9,
        char: 'ᆽ',
    },
    KeysymChar {
        keysym: 0x0eea,
        char: 'ᆾ',
    },
    KeysymChar {
        keysym: 0x0eeb,
        char: 'ᆿ',
    },
    KeysymChar {
        keysym: 0x0eec,
        char: 'ᇀ',
    },
    KeysymChar {
        keysym: 0x0eed,
        char: 'ᇁ',
    },
    KeysymChar {
        keysym: 0x0eee,
        char: 'ᇂ',
    },
    KeysymChar {
        keysym: 0x0eef,
        char: 'ㅭ',
    },
    KeysymChar {
        keysym: 0x0ef0,
        char: 'ㅱ',
    },
    KeysymChar {
        keysym: 0x0ef1,
        char: 'ㅸ',
    },
    KeysymChar {
        keysym: 0x0ef2,
        char: 'ㅿ',
    },
    KeysymChar {
        keysym: 0x0ef3,
        char: 'ㆁ',
    },
    KeysymChar {
        keysym: 0x0ef4,
        char: 'ㆄ',
    },
    KeysymChar {
        keysym: 0x0ef5,
        char: 'ㆆ',
    },
    KeysymChar {
        keysym: 0x0ef6,
        char: 'ㆍ',
    },
    KeysymChar {
        keysym: 0x0ef7,
        char: 'ㆎ',
    },
    KeysymChar {
        keysym: 0x0ef8,
        char: 'ᇫ',
    },
    KeysymChar {
        keysym: 0x0ef9,
        char: 'ᇰ',
    },
    KeysymChar {
        keysym: 0x0efa,
        char: 'ᇹ',
    },
    KeysymChar {
        keysym: 0x0eff,
        char: '₩',
    },
    KeysymChar {
        keysym: 0x13bc,
        char: 'Œ',
    },
    KeysymChar {
        keysym: 0x13bd,
        char: 'œ',
    },
    KeysymChar {
        keysym: 0x13be,
        char: 'Ÿ',
    },
    KeysymChar {
        keysym: 0x20ac,
        char: '€',
    },
    KeysymChar {
        keysym: 0xff08,
        char: '\u{8}',
    },
    KeysymChar {
        keysym: 0xff09,
        char: '\t',
    },
    KeysymChar {
        keysym: 0xff0a,
        char: '\n',
    },
    KeysymChar {
        keysym: 0xff0b,
        char: '\u{b}',
    },
    KeysymChar {
        keysym: 0xff0d,
        char: '\r',
    },
    KeysymChar {
        keysym: 0xff1b,
        char: '\u{1b}',
    },
    KeysymChar {
        keysym: 0xff80,
        char: ' ',
    },
    KeysymChar {
        keysym: 0xff89,
        char: '\t',
    },
    KeysymChar {
        keysym: 0xff8d,
        char: '\r',
    },
    KeysymChar {
        keysym: 0xffaa,
        char: '*',
    },
    KeysymChar {
        keysym: 0xffab,
        char: '+',
    },
    KeysymChar {
        keysym: 0xffac,
        char: ',',
    },
    KeysymChar {
        keysym: 0xffad,
        char: '-',
    },
    KeysymChar {
        keysym: 0xffae,
        char: '.',
    },
    KeysymChar {
        keysym: 0xffaf,
        char: '/',
    },
    KeysymChar {
        keysym: 0xffb0,
        char: '0',
    },
    KeysymChar {
        keysym: 0xffb1,
        char: '1',
    },
    KeysymChar {
        keysym: 0xffb2,
        char: '2',
    },
    KeysymChar {
        keysym: 0xffb3,
        char: '3',
    },
    KeysymChar {
        keysym: 0xffb4,
        char: '4',
    },
    KeysymChar {
        keysym: 0xffb5,
        char: '5',
    },
    KeysymChar {
        keysym: 0xffb6,
        char: '6',
    },
    KeysymChar {
        keysym: 0xffb7,
        char: '7',
    },
    KeysymChar {
        keysym: 0xffb8,
        char: '8',
    },
    KeysymChar {
        keysym: 0xffb9,
        char: '9',
    },
    KeysymChar {
        keysym: 0xffbd,
        char: '=',
    },
    KeysymChar {
        keysym: 0xffff,
        char: '\u{7f}',
    },
];

pub(super) static KEYSYM32_TO_CHAR: &[Keysym32Char] = &[
    Keysym32Char {
        keysym: 0x10081200,
        char: '0',
    },
    Keysym32Char {
        keysym: 0x10081201,
        char: '1',
    },
    Keysym32Char {
        keysym: 0x10081202,
        char: '2',
    },
    Keysym32Char {
        keysym: 0x10081203,
        char: '3',
    },
    Keysym32Char {
        keysym: 0x10081204,
        char: '4',
    },
    Keysym32Char {
        keysym: 0x10081205,
        char: '5',
    },
    Keysym32Char {
        keysym: 0x10081206,
        char: '6',
    },
    Keysym32Char {
        keysym: 0x10081207,
        char: '7',
    },
    Keysym32Char {
        keysym: 0x10081208,
        char: '8',
    },
    Keysym32Char {
        keysym: 0x10081209,
        char: '9',
    },
    Keysym32Char {
        keysym: 0x1008120a,
        char: '*',
    },
    Keysym32Char {
        keysym: 0x1008120b,
        char: '#',
    },
];

pub(super) static CHAR_TO_BESPOKE_IDX: PhfMap<char, u16> = {
    let Ok(map) = PhfMap::new(10656210946825422025, &[(0, 334), (0, 73), (0, 4), (0, 42), (0, 6), (0, 0), (0, 7), (0, 4), (0, 0), (0, 1), (0, 0), (0, 2), (0, 1), (0, 2), (0, 101), (0, 19), (0, 6), (0, 104), (0, 2), (0, 0), (0, 16), (0, 15), (0, 2), (0, 79), (0, 9), (0, 3), (0, 22), (0, 64), (0, 9), (0, 1), (0, 69), (0, 6), (0, 0), (0, 116), (0, 4), (0, 11), (0, 30), (0, 65), (0, 0), (0, 101), (0, 4), (0, 42), (0, 41), (0, 5), (0, 2), (0, 94), (0, 168), (0, 21), (0, 57), (0, 31), (0, 13), (0, 8), (0, 1), (0, 191), (0, 69), (0, 188), (0, 153), (0, 10), (0, 10), (0, 17), (0, 17), (0, 5), (0, 164), (0, 5), (0, 12), (0, 74), (0, 13), (0, 0), (0, 14), (0, 0), (0, 0), (0, 283), (0, 12), (0, 32), (0, 73), (0, 0), (0, 0), (0, 1), (0, 4), (0, 0), (0, 0), (0, 3), (0, 0), (0, 99), (0, 1), (0, 0), (0, 61), (0, 33), (0, 39), (0, 31), (0, 10), (0, 0), (0, 0), (0, 14), (0, 1), (0, 0), (0, 1), (0, 7), (0, 24), (0, 33), (0, 87), (0, 14), (0, 23), (0, 1), (0, 1), (0, 2), (0, 144), (0, 95), (0, 4), (0, 10), (0, 224), (0, 108), (0, 6), (0, 101), (0, 1), (0, 19), (0, 2), (0, 8), (0, 151), (0, 0), (0, 88), (0, 69), (0, 192), (0, 54), (0, 190), (0, 77), (0, 0), (0, 8), (0, 284), (0, 33), (0, 86), (0, 771), (0, 150), (0, 117), (0, 187), (0, 233), (0, 656), (0, 54), (0, 13), (0, 434), (0, 0), (0, 103), (0, 29), (0, 22), (0, 362), (0, 59), (0, 4), (0, 141), (0, 23), (0, 0), (0, 595), (0, 339), (0, 173), (0, 50), (0, 1), (0, 1), (0, 3), (0, 770), (0, 38), (0, 396), (0, 66), (0, 1), (0, 1), (0, 10), (0, 0), (0, 294), (0, 157), (0, 0), (0, 10), (0, 16), (0, 652), (0, 212), (0, 115), (0, 2), (0, 728), (0, 0), (0, 1), (0, 10), (0, 4), (0, 44), (0, 0), (0, 0), (0, 105), (0, 2), (0, 2), (0, 4), (0, 203), (0, 19), (0, 0), (0, 47), (0, 1), (0, 137), (0, 32), (0, 303), (0, 146), (0, 98), (0, 0), (1, 164), (1, 0), (0, 43), (0, 3), (0, 45), (0, 0), (0, 31), (0, 69), (0, 39), (0, 181), (0, 2), (0, 690), (0, 184), (0, 7), (0, 29), (0, 17), (0, 26), (0, 3), (0, 21), (0, 718), (1, 54), (1, 648), (0, 38), (5, 537), (0, 320), (0, 615), (0, 0), (3, 147), (0, 613), (0, 248), (2, 163), (0, 22), (0, 10), (0, 347), (0, 38), (0, 71), (0, 15), (0, 0), (0, 143), (0, 39), (0, 14), (0, 57), (0, 670), (0, 1), (1, 651), (0, 282), (0, 18), (2, 432), (0, 89), (0, 2), (0, 88), (0, 3), (0, 0), (0, 118), (0, 386), (2, 258), (1, 763), (4, 766), (6, 661)], &[656, 107, 383, 574, 270, 175, 188, 682, 820, 554, 957, 307, 338, 411, 880, 727, 987, 399, 54, 945, 846, 517, 1000, 1, 851, 979, 606, 630, 600, 565, 76, 871, 343, 741, 124, 49, 892, 426, 721, 974, 19, 787, 571, 749, 665, 408, 627, 118, 374, 720, 347, 250, 937, 657, 492, 485, 458, 10, 907, 610, 755, 859, 661, 752, 391, 427, 73, 197, 538, 242, 108, 1010, 731, 165, 252, 489, 34, 8, 450, 510, 273, 139, 303, 432, 444, 251, 735, 309, 247, 227, 221, 1196, 241, 439, 51, 919, 588, 718, 525, 739, 711, 147, 27, 85, 938, 90, 493, 535, 753, 238, 65, 580, 451, 876, 392, 473, 915, 835, 785, 253, 594, 341, 101, 172, 15, 302, 799, 278, 923, 233, 185, 962, 530, 484, 888, 81, 501, 317, 558, 585, 503, 951, 59, 231, 24, 201, 889, 533, 319, 176, 217, 520, 351, 692, 640, 405, 545, 282, 38, 906, 189, 115, 220, 717, 946, 28, 905, 772, 136, 291, 702, 72, 494, 335, 1398, 852, 700, 703, 645, 516, 203, 109, 400, 623, 765, 843, 967, 976, 194, 671, 435, 453, 368, 980, 1006, 498, 649, 263, 568, 801, 20, 289, 740, 959, 380, 62, 754, 286, 582, 643, 603, 149, 984, 680, 614, 413, 22, 816, 202, 56, 344, 567, 96, 3, 283, 576, 793, 651, 459, 689, 881, 381, 663, 893, 882, 230, 410, 660, 860, 157, 144, 487, 832, 515, 143, 894, 169, 476, 513, 981, 887, 607, 762, 111, 724, 873, 546, 324, 812, 758, 293, 87, 870, 781, 868, 353, 589, 355, 64, 334, 348, 931, 631, 514, 792, 591, 462, 102, 82, 12, 310, 909, 924, 401, 123, 948, 130, 505, 91, 620, 508, 357, 21, 865, 628, 939, 491, 511, 342, 199, 339, 294, 274, 652, 840, 45, 521, 822, 989, 902, 653, 647, 215, 288, 861, 212, 534, 708, 921, 78, 611, 37, 746, 183, 133, 900, 285, 442, 53, 1189, 766, 956, 190, 495, 164, 696, 416, 968, 224, 837, 750, 29, 764, 394, 997, 596, 802, 17, 769, 174, 145, 667, 615, 187, 532, 1002, 691, 208, 370, 560, 751, 482, 292, 916, 216, 953, 277, 229, 681, 557, 883, 434, 475, 257, 809, 479, 407, 332, 784, 573, 854, 690, 932, 103, 191, 969, 30, 35, 308, 467, 760, 255, 705, 138, 356, 685, 982, 325, 402, 234, 254, 605, 540, 454, 112, 367, 232, 845, 75, 117, 196, 431, 549, 104, 608, 878, 536, 624, 207, 206, 867, 58, 84, 683, 986, 168, 480, 961, 419, 152, 316, 728, 457, 61, 895, 284, 259, 912, 862, 421, 940, 158, 92, 236, 803, 358, 579, 578, 146, 650, 933, 509, 488, 122, 570, 918, 295, 697, 267, 839, 345, 430, 5, 757, 768, 260, 593, 599, 834, 437, 996, 625, 814, 616, 320, 80, 817, 113, 896, 668, 518, 826, 89, 50, 23, 218, 925, 161, 744, 547, 386, 522, 581, 44, 838, 125, 779, 237, 584, 465, 36, 14, 428, 911, 736, 701, 162, 875, 941, 93, 529, 276, 120, 359, 613, 223, 245, 583, 553, 209, 395, 182, 975, 304, 601, 992, 179, 885, 523, 244, 963, 204, 559, 301, 693, 670, 786, 632, 55, 68, 677, 994, 710, 904, 366, 655, 240, 375, 958, 1005, 481, 184, 1191, 842, 167, 95, 695, 97, 382, 738, 412, 326, 855, 497, 279, 715, 995, 927, 805, 819, 2, 926, 884, 423, 729, 192, 336, 105, 684, 328, 519, 69, 983, 171, 153, 132, 63, 31, 477, 456, 694, 178, 950, 763, 180, 658, 712, 213, 524, 142, 362, 872, 575, 999, 404, 934, 722, 971, 32, 265, 119, 821, 572, 506, 271, 747, 321, 388, 811, 502, 300, 512, 662, 897, 114, 646, 890, 564, 543, 550, 140, 6, 198, 305, 372, 687, 707, 732, 528, 828, 11, 771, 504, 211, 970, 869, 1001, 337, 77, 644, 551, 641, 39, 743, 418, 248, 287, 908, 346, 609, 1007, 733, 942, 296, 371, 94, 947, 719, 988, 815, 129, 314, 935, 539, 490, 669, 377, 848, 886, 7, 396, 269, 349, 389, 66, 311, 836, 563, 595, 299, 898, 648, 964, 52, 173, 86, 794, 46, 313, 877, 258, 318, 955, 704, 360, 991, 847, 555, 25, 417, 163, 952, 40, 1188, 393, 384, 856, 249, 177, 205, 148, 350, 361, 406, 913, 16, 642, 928, 672, 186, 312, 849, 617, 998, 409, 70, 943, 424, 531, 246, 448, 460, 378, 699, 844, 767, 977, 329, 397, 474, 106, 385, 761, 363, 127, 561, 415, 298, 725, 116, 98, 369, 33, 972, 181, 569, 43, 57, 759, 154, 1192, 315, 471, 866, 364, 214, 306, 742, 659, 137, 499, 507, 770, 373, 552, 857, 151, 745, 195, 71, 829, 542, 4, 83, 679, 698, 1009, 420, 541, 598, 810, 929, 853, 219, 586, 891, 155, 48, 141, 879, 688, 414, 433, 425, 1008, 379, 222, 526, 47, 340, 446, 42, 60, 874, 813, 272, 936, 436, 88, 824, 686, 973, 590, 917, 901, 243, 469, 920, 390, 676, 748, 327, 235, 791, 18, 864, 99, 79, 13, 795, 621, 226, 297, 592, 612, 833, 483, 965, 200, 807, 121, 604, 556, 899, 275, 949, 160, 1004, 664, 960, 629, 486, 587, 734, 990, 228, 156, 403, 170, 463, 910, 678, 440, 756, 944, 674, 1003, 709, 239, 131, 74, 135, 262, 134, 850, 675, 398, 903, 783, 978, 331, 268, 496, 429, 966, 841, 26, 914, 566, 985, 597, 562, 730, 544, 537, 993, 290, 100, 673, 654, 41, 797, 256, 863, 387, 1190, 706, 376, 225, 330, 266, 210, 261, 954, 280, 323, 264, 500, 527, 858, 452, 626, 713, 922, 930, 830]);

    map
};

pub(super) static DATAS: &[KeysymData] = &[
    KeysymData {
        keysym_or_definitive_idx: 0x00000000,
        name_start: 0,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000020,
        name_start: 8,
        name_len: 5,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000021,
        name_start: 13,
        name_len: 6,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000022,
        name_start: 19,
        name_len: 8,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000023,
        name_start: 27,
        name_len: 10,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000024,
        name_start: 37,
        name_len: 6,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000025,
        name_start: 43,
        name_len: 7,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000026,
        name_start: 50,
        name_len: 9,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000027,
        name_start: 59,
        name_len: 10,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000008,
        name_start: 69,
        name_len: 10,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000028,
        name_start: 79,
        name_len: 9,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000029,
        name_start: 88,
        name_len: 10,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000002a,
        name_start: 98,
        name_len: 8,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000002b,
        name_start: 106,
        name_len: 4,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000002c,
        name_start: 110,
        name_len: 5,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000002d,
        name_start: 115,
        name_len: 5,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000002e,
        name_start: 120,
        name_len: 6,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000002f,
        name_start: 126,
        name_len: 5,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000030,
        name_start: 131,
        name_len: 1,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000031,
        name_start: 132,
        name_len: 1,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000032,
        name_start: 133,
        name_len: 1,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000033,
        name_start: 134,
        name_len: 1,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000034,
        name_start: 135,
        name_len: 1,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000035,
        name_start: 136,
        name_len: 1,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000036,
        name_start: 137,
        name_len: 1,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000037,
        name_start: 138,
        name_len: 1,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000038,
        name_start: 139,
        name_len: 1,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000039,
        name_start: 140,
        name_len: 1,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000003a,
        name_start: 141,
        name_len: 5,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000003b,
        name_start: 146,
        name_len: 9,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000003c,
        name_start: 155,
        name_len: 4,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000003d,
        name_start: 159,
        name_len: 5,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000003e,
        name_start: 164,
        name_len: 7,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000003f,
        name_start: 171,
        name_len: 8,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000040,
        name_start: 179,
        name_len: 2,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000041,
        name_start: 181,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000042,
        name_start: 182,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000043,
        name_start: 183,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000044,
        name_start: 184,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000045,
        name_start: 185,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000046,
        name_start: 186,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000047,
        name_start: 187,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000048,
        name_start: 188,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000049,
        name_start: 189,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000004a,
        name_start: 190,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000004b,
        name_start: 191,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000004c,
        name_start: 192,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000004d,
        name_start: 193,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000004e,
        name_start: 194,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000004f,
        name_start: 195,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000050,
        name_start: 196,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000051,
        name_start: 197,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000052,
        name_start: 198,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000053,
        name_start: 199,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000054,
        name_start: 200,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000055,
        name_start: 201,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000056,
        name_start: 202,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000057,
        name_start: 203,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000058,
        name_start: 204,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000059,
        name_start: 205,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000005a,
        name_start: 206,
        name_len: 1,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000005b,
        name_start: 207,
        name_len: 11,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000005c,
        name_start: 218,
        name_len: 9,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000005d,
        name_start: 227,
        name_len: 12,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000005e,
        name_start: 239,
        name_len: 11,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000005f,
        name_start: 250,
        name_len: 10,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000060,
        name_start: 260,
        name_len: 5,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000042,
        name_start: 265,
        name_len: 9,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000061,
        name_start: 274,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000062,
        name_start: 275,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000063,
        name_start: 276,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000064,
        name_start: 277,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000065,
        name_start: 278,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000066,
        name_start: 279,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000067,
        name_start: 280,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000068,
        name_start: 281,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000069,
        name_start: 282,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000006a,
        name_start: 283,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000006b,
        name_start: 284,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000006c,
        name_start: 285,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000006d,
        name_start: 286,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000006e,
        name_start: 287,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000006f,
        name_start: 288,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000070,
        name_start: 289,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000071,
        name_start: 290,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000072,
        name_start: 291,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000073,
        name_start: 292,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000074,
        name_start: 293,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000075,
        name_start: 294,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000076,
        name_start: 295,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000077,
        name_start: 296,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000078,
        name_start: 297,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000079,
        name_start: 298,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000007a,
        name_start: 299,
        name_len: 1,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000007b,
        name_start: 300,
        name_len: 9,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000007c,
        name_start: 309,
        name_len: 3,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000007d,
        name_start: 312,
        name_len: 10,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000007e,
        name_start: 322,
        name_len: 10,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000a0,
        name_start: 332,
        name_len: 12,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000a1,
        name_start: 344,
        name_len: 10,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000a2,
        name_start: 354,
        name_len: 4,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000a3,
        name_start: 358,
        name_len: 8,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000a4,
        name_start: 366,
        name_len: 8,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000a5,
        name_start: 374,
        name_len: 3,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000a6,
        name_start: 377,
        name_len: 9,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000a7,
        name_start: 386,
        name_len: 7,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000a8,
        name_start: 393,
        name_len: 9,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000a9,
        name_start: 402,
        name_len: 9,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000aa,
        name_start: 411,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ab,
        name_start: 422,
        name_len: 13,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000006d,
        name_start: 435,
        name_len: 13,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ac,
        name_start: 448,
        name_len: 7,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ad,
        name_start: 455,
        name_len: 6,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ae,
        name_start: 461,
        name_len: 10,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000af,
        name_start: 471,
        name_len: 6,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000b0,
        name_start: 477,
        name_len: 6,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000b1,
        name_start: 483,
        name_len: 9,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000b2,
        name_start: 492,
        name_len: 11,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000b3,
        name_start: 503,
        name_len: 13,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000b4,
        name_start: 516,
        name_len: 5,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000b5,
        name_start: 521,
        name_len: 2,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000b6,
        name_start: 523,
        name_len: 9,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000b7,
        name_start: 532,
        name_len: 14,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000b8,
        name_start: 546,
        name_len: 7,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000b9,
        name_start: 553,
        name_len: 11,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ba,
        name_start: 564,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000007d,
        name_start: 573,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000bb,
        name_start: 585,
        name_len: 14,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000007f,
        name_start: 599,
        name_len: 14,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000bc,
        name_start: 613,
        name_len: 10,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000bd,
        name_start: 623,
        name_len: 7,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000be,
        name_start: 630,
        name_len: 13,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000bf,
        name_start: 643,
        name_len: 12,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000c0,
        name_start: 655,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000c1,
        name_start: 661,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000c2,
        name_start: 667,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000c3,
        name_start: 678,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000c4,
        name_start: 684,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000c5,
        name_start: 694,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000c6,
        name_start: 699,
        name_len: 2,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000c7,
        name_start: 701,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000c8,
        name_start: 709,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000c9,
        name_start: 715,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ca,
        name_start: 721,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000cb,
        name_start: 732,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000cc,
        name_start: 742,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000cd,
        name_start: 748,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ce,
        name_start: 754,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000cf,
        name_start: 765,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000d0,
        name_start: 775,
        name_len: 3,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000095,
        name_start: 778,
        name_len: 3,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000d1,
        name_start: 781,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000d2,
        name_start: 787,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000d3,
        name_start: 793,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000d4,
        name_start: 799,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000d5,
        name_start: 810,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000d6,
        name_start: 816,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000d7,
        name_start: 826,
        name_len: 8,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000d8,
        name_start: 834,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000009e,
        name_start: 840,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000d9,
        name_start: 848,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000da,
        name_start: 854,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000db,
        name_start: 860,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000dc,
        name_start: 871,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000dd,
        name_start: 881,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000de,
        name_start: 887,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000a5,
        name_start: 892,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR | KEYSYM_IS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000df,
        name_start: 897,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000e0,
        name_start: 903,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000e1,
        name_start: 909,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000e2,
        name_start: 915,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000e3,
        name_start: 926,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000e4,
        name_start: 932,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000e5,
        name_start: 942,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000e6,
        name_start: 947,
        name_len: 2,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000e7,
        name_start: 949,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000e8,
        name_start: 957,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000e9,
        name_start: 963,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ea,
        name_start: 969,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000eb,
        name_start: 980,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ec,
        name_start: 990,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ed,
        name_start: 996,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ee,
        name_start: 1002,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ef,
        name_start: 1013,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000f0,
        name_start: 1023,
        name_len: 3,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000f1,
        name_start: 1026,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000f2,
        name_start: 1032,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000f3,
        name_start: 1038,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000f4,
        name_start: 1044,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000f5,
        name_start: 1055,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000f6,
        name_start: 1061,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000f7,
        name_start: 1071,
        name_len: 8,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000f8,
        name_start: 1079,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000c0,
        name_start: 1085,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000f9,
        name_start: 1093,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000fa,
        name_start: 1099,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000fb,
        name_start: 1105,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000fc,
        name_start: 1116,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000fd,
        name_start: 1126,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000fe,
        name_start: 1132,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000000ff,
        name_start: 1137,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001a1,
        name_start: 1147,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001a2,
        name_start: 1154,
        name_len: 5,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001a3,
        name_start: 1159,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001a5,
        name_start: 1166,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001a6,
        name_start: 1172,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001a9,
        name_start: 1178,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001aa,
        name_start: 1184,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001ab,
        name_start: 1192,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001ac,
        name_start: 1198,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001ae,
        name_start: 1204,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001af,
        name_start: 1210,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001b1,
        name_start: 1219,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001b2,
        name_start: 1226,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001b3,
        name_start: 1232,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001b5,
        name_start: 1239,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001b6,
        name_start: 1245,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001b7,
        name_start: 1251,
        name_len: 5,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001b9,
        name_start: 1256,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001ba,
        name_start: 1262,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001bb,
        name_start: 1270,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001bc,
        name_start: 1276,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001bd,
        name_start: 1282,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001be,
        name_start: 1293,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001bf,
        name_start: 1299,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001c0,
        name_start: 1308,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001c3,
        name_start: 1314,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001c5,
        name_start: 1320,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001c6,
        name_start: 1326,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001c8,
        name_start: 1332,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001ca,
        name_start: 1338,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001cc,
        name_start: 1345,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001cf,
        name_start: 1351,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001d0,
        name_start: 1357,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001d1,
        name_start: 1364,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001d2,
        name_start: 1370,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001d5,
        name_start: 1376,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001d8,
        name_start: 1388,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001d9,
        name_start: 1394,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001db,
        name_start: 1399,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001de,
        name_start: 1411,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001e0,
        name_start: 1419,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001e3,
        name_start: 1425,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001e5,
        name_start: 1431,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001e6,
        name_start: 1437,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001e8,
        name_start: 1443,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001ea,
        name_start: 1449,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001ec,
        name_start: 1456,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001ef,
        name_start: 1462,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001f0,
        name_start: 1468,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001f1,
        name_start: 1475,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001f2,
        name_start: 1481,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001f5,
        name_start: 1487,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001f8,
        name_start: 1499,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001f9,
        name_start: 1505,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001fb,
        name_start: 1510,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001fe,
        name_start: 1522,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001ff,
        name_start: 1530,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002a1,
        name_start: 1538,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002a6,
        name_start: 1545,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002a9,
        name_start: 1556,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002ab,
        name_start: 1565,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002ac,
        name_start: 1571,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002b1,
        name_start: 1582,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002b6,
        name_start: 1589,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002b9,
        name_start: 1600,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002bb,
        name_start: 1608,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002bc,
        name_start: 1614,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002c5,
        name_start: 1625,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002c6,
        name_start: 1634,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002d5,
        name_start: 1645,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002d8,
        name_start: 1654,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002dd,
        name_start: 1665,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002de,
        name_start: 1671,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002e5,
        name_start: 1682,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002e6,
        name_start: 1691,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002f5,
        name_start: 1702,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002f8,
        name_start: 1711,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002fd,
        name_start: 1722,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000002fe,
        name_start: 1728,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003a2,
        name_start: 1739,
        name_len: 3,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000118,
        name_start: 1742,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003a3,
        name_start: 1747,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003a5,
        name_start: 1755,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003a6,
        name_start: 1761,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003aa,
        name_start: 1769,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003ab,
        name_start: 1776,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003ac,
        name_start: 1784,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003b3,
        name_start: 1790,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003b5,
        name_start: 1798,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003b6,
        name_start: 1804,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003ba,
        name_start: 1812,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003bb,
        name_start: 1819,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003bc,
        name_start: 1827,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003bd,
        name_start: 1833,
        name_len: 3,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003bf,
        name_start: 1836,
        name_len: 3,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003c0,
        name_start: 1839,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003c7,
        name_start: 1846,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003cc,
        name_start: 1853,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003cf,
        name_start: 1862,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003d1,
        name_start: 1869,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003d2,
        name_start: 1877,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003d3,
        name_start: 1884,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003d9,
        name_start: 1892,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003dd,
        name_start: 1899,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003de,
        name_start: 1905,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003e0,
        name_start: 1912,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003e7,
        name_start: 1919,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003ec,
        name_start: 1926,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003ef,
        name_start: 1935,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003f1,
        name_start: 1942,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003f2,
        name_start: 1950,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003f3,
        name_start: 1957,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003f9,
        name_start: 1965,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003fd,
        name_start: 1972,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000003fe,
        name_start: 1978,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000047e,
        name_start: 1985,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004a1,
        name_start: 1993,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004a2,
        name_start: 2006,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004a3,
        name_start: 2025,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004a4,
        name_start: 2044,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004a5,
        name_start: 2054,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000141,
        name_start: 2070,
        name_len: 14,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004a6,
        name_start: 2084,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004a7,
        name_start: 2091,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004a8,
        name_start: 2097,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004a9,
        name_start: 2103,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004aa,
        name_start: 2109,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004ab,
        name_start: 2115,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004ac,
        name_start: 2121,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004ad,
        name_start: 2128,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004ae,
        name_start: 2135,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004af,
        name_start: 2142,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000014c,
        name_start: 2150,
        name_len: 7,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004b0,
        name_start: 2157,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004b1,
        name_start: 2171,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004b2,
        name_start: 2177,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004b3,
        name_start: 2183,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004b4,
        name_start: 2189,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004b5,
        name_start: 2195,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004b6,
        name_start: 2201,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004b7,
        name_start: 2208,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004b8,
        name_start: 2215,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004b9,
        name_start: 2222,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004ba,
        name_start: 2229,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004bb,
        name_start: 2236,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004bc,
        name_start: 2243,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004bd,
        name_start: 2251,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004be,
        name_start: 2258,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004bf,
        name_start: 2265,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c0,
        name_start: 2272,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c1,
        name_start: 2279,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000015f,
        name_start: 2287,
        name_len: 7,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c2,
        name_start: 2294,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000161,
        name_start: 2302,
        name_len: 7,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c3,
        name_start: 2309,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c4,
        name_start: 2316,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c5,
        name_start: 2323,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c6,
        name_start: 2330,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c7,
        name_start: 2337,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c8,
        name_start: 2344,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c9,
        name_start: 2351,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004ca,
        name_start: 2358,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004cb,
        name_start: 2365,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004cc,
        name_start: 2372,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000016c,
        name_start: 2379,
        name_len: 7,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004cd,
        name_start: 2386,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004ce,
        name_start: 2393,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004cf,
        name_start: 2400,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d0,
        name_start: 2407,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d1,
        name_start: 2414,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d2,
        name_start: 2421,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d3,
        name_start: 2428,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d4,
        name_start: 2435,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d5,
        name_start: 2442,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d6,
        name_start: 2449,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d7,
        name_start: 2456,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d8,
        name_start: 2463,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d9,
        name_start: 2470,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004da,
        name_start: 2477,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004db,
        name_start: 2484,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004dc,
        name_start: 2491,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004dd,
        name_start: 2498,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004de,
        name_start: 2504,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004df,
        name_start: 2515,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005ac,
        name_start: 2530,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005bb,
        name_start: 2542,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005bf,
        name_start: 2558,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005c1,
        name_start: 2578,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005c2,
        name_start: 2590,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005c3,
        name_start: 2608,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005c4,
        name_start: 2626,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005c5,
        name_start: 2643,
        name_len: 21,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005c6,
        name_start: 2664,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005c7,
        name_start: 2681,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005c8,
        name_start: 2692,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005c9,
        name_start: 2702,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005ca,
        name_start: 2719,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005cb,
        name_start: 2729,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005cc,
        name_start: 2740,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005cd,
        name_start: 2751,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005ce,
        name_start: 2761,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005cf,
        name_start: 2772,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005d0,
        name_start: 2782,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005d1,
        name_start: 2793,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005d2,
        name_start: 2802,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005d3,
        name_start: 2813,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005d4,
        name_start: 2824,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005d5,
        name_start: 2836,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005d6,
        name_start: 2846,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005d7,
        name_start: 2856,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005d8,
        name_start: 2866,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005d9,
        name_start: 2876,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005da,
        name_start: 2886,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005e0,
        name_start: 2898,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005e1,
        name_start: 2912,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005e2,
        name_start: 2922,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005e3,
        name_start: 2932,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005e4,
        name_start: 2942,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005e5,
        name_start: 2952,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005e6,
        name_start: 2963,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005e7,
        name_start: 2974,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001a5,
        name_start: 2983,
        name_len: 10,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005e8,
        name_start: 2993,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005e9,
        name_start: 3003,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005ea,
        name_start: 3021,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005eb,
        name_start: 3031,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005ec,
        name_start: 3046,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005ed,
        name_start: 3061,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005ee,
        name_start: 3076,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005ef,
        name_start: 3088,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005f0,
        name_start: 3100,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005f1,
        name_start: 3112,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005f2,
        name_start: 3125,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006a1,
        name_start: 3137,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006a2,
        name_start: 3148,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006a3,
        name_start: 3161,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006a4,
        name_start: 3172,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001b5,
        name_start: 3184,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006a5,
        name_start: 3195,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006a6,
        name_start: 3208,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001b8,
        name_start: 3219,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006a7,
        name_start: 3229,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001ba,
        name_start: 3241,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006a8,
        name_start: 3252,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001bc,
        name_start: 3263,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006a9,
        name_start: 3273,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001be,
        name_start: 3285,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006aa,
        name_start: 3296,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001c0,
        name_start: 3308,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ab,
        name_start: 3319,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ac,
        name_start: 3331,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ad,
        name_start: 3344,
        name_len: 25,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ae,
        name_start: 3369,
        name_len: 19,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006af,
        name_start: 3388,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001c6,
        name_start: 3401,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006b0,
        name_start: 3412,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006b1,
        name_start: 3422,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006b2,
        name_start: 3433,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006b3,
        name_start: 3446,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006b4,
        name_start: 3457,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001cc,
        name_start: 3469,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006b5,
        name_start: 3480,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006b6,
        name_start: 3493,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001cf,
        name_start: 3504,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006b7,
        name_start: 3514,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001d1,
        name_start: 3526,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006b8,
        name_start: 3537,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001d3,
        name_start: 3548,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006b9,
        name_start: 3558,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001d5,
        name_start: 3570,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ba,
        name_start: 3581,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001d7,
        name_start: 3593,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006bb,
        name_start: 3604,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006bc,
        name_start: 3616,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006bd,
        name_start: 3629,
        name_len: 25,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006be,
        name_start: 3654,
        name_len: 19,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006bf,
        name_start: 3673,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000001dd,
        name_start: 3686,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006c0,
        name_start: 3697,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006c1,
        name_start: 3708,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006c2,
        name_start: 3718,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006c3,
        name_start: 3729,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006c4,
        name_start: 3741,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006c5,
        name_start: 3752,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006c6,
        name_start: 3763,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006c7,
        name_start: 3774,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006c8,
        name_start: 3786,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006c9,
        name_start: 3797,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ca,
        name_start: 3807,
        name_len: 15,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006cb,
        name_start: 3822,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006cc,
        name_start: 3833,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006cd,
        name_start: 3844,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ce,
        name_start: 3855,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006cf,
        name_start: 3866,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006d0,
        name_start: 3876,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006d1,
        name_start: 3887,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006d2,
        name_start: 3898,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006d3,
        name_start: 3909,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006d4,
        name_start: 3920,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006d5,
        name_start: 3931,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006d6,
        name_start: 3941,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006d7,
        name_start: 3953,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006d8,
        name_start: 3964,
        name_len: 17,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006d9,
        name_start: 3981,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006da,
        name_start: 3994,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006db,
        name_start: 4005,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006dc,
        name_start: 4017,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006dd,
        name_start: 4027,
        name_len: 14,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006de,
        name_start: 4041,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006df,
        name_start: 4053,
        name_len: 17,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006e0,
        name_start: 4070,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006e1,
        name_start: 4081,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006e2,
        name_start: 4091,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006e3,
        name_start: 4102,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006e4,
        name_start: 4114,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006e5,
        name_start: 4125,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006e6,
        name_start: 4136,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006e7,
        name_start: 4147,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006e8,
        name_start: 4159,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006e9,
        name_start: 4170,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ea,
        name_start: 4180,
        name_len: 15,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006eb,
        name_start: 4195,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ec,
        name_start: 4206,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ed,
        name_start: 4217,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ee,
        name_start: 4228,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ef,
        name_start: 4239,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006f0,
        name_start: 4249,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006f1,
        name_start: 4260,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006f2,
        name_start: 4271,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006f3,
        name_start: 4282,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006f4,
        name_start: 4293,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006f5,
        name_start: 4304,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006f6,
        name_start: 4314,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006f7,
        name_start: 4326,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006f8,
        name_start: 4337,
        name_len: 17,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006f9,
        name_start: 4354,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006fa,
        name_start: 4367,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006fb,
        name_start: 4378,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006fc,
        name_start: 4390,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006fd,
        name_start: 4400,
        name_len: 14,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006fe,
        name_start: 4414,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000006ff,
        name_start: 4426,
        name_len: 17,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007a1,
        name_start: 4443,
        name_len: 17,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007a2,
        name_start: 4460,
        name_len: 19,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007a3,
        name_start: 4479,
        name_len: 15,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007a4,
        name_start: 4494,
        name_len: 16,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007a5,
        name_start: 4510,
        name_len: 18,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000223,
        name_start: 4528,
        name_len: 19,
        flags: 0 | IS_UPPER | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007a7,
        name_start: 4547,
        name_len: 19,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007a8,
        name_start: 4566,
        name_len: 19,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007a9,
        name_start: 4585,
        name_len: 21,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007ab,
        name_start: 4606,
        name_len: 17,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007ae,
        name_start: 4623,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007af,
        name_start: 4643,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007b1,
        name_start: 4657,
        name_len: 17,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007b2,
        name_start: 4674,
        name_len: 19,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007b3,
        name_start: 4693,
        name_len: 15,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007b4,
        name_start: 4708,
        name_len: 16,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007b5,
        name_start: 4724,
        name_len: 18,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007b6,
        name_start: 4742,
        name_len: 24,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007b7,
        name_start: 4766,
        name_len: 19,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007b8,
        name_start: 4785,
        name_len: 19,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007b9,
        name_start: 4804,
        name_len: 21,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007ba,
        name_start: 4825,
        name_len: 27,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007bb,
        name_start: 4852,
        name_len: 17,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007c1,
        name_start: 4869,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007c2,
        name_start: 4880,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007c3,
        name_start: 4890,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007c4,
        name_start: 4901,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007c5,
        name_start: 4912,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007c6,
        name_start: 4925,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007c7,
        name_start: 4935,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007c8,
        name_start: 4944,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007c9,
        name_start: 4955,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007ca,
        name_start: 4965,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007cb,
        name_start: 4976,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000240,
        name_start: 4987,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007cc,
        name_start: 4999,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007cd,
        name_start: 5007,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007ce,
        name_start: 5015,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007cf,
        name_start: 5023,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007d0,
        name_start: 5036,
        name_len: 8,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007d1,
        name_start: 5044,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007d2,
        name_start: 5053,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007d4,
        name_start: 5064,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007d5,
        name_start: 5073,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007d6,
        name_start: 5086,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007d7,
        name_start: 5095,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007d8,
        name_start: 5104,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007d9,
        name_start: 5113,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007e1,
        name_start: 5124,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007e2,
        name_start: 5135,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007e3,
        name_start: 5145,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007e4,
        name_start: 5156,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007e5,
        name_start: 5167,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007e6,
        name_start: 5180,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007e7,
        name_start: 5190,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007e8,
        name_start: 5199,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007e9,
        name_start: 5210,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007ea,
        name_start: 5220,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007eb,
        name_start: 5231,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000259,
        name_start: 5242,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007ec,
        name_start: 5254,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007ed,
        name_start: 5262,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007ee,
        name_start: 5270,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007ef,
        name_start: 5278,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007f0,
        name_start: 5291,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007f1,
        name_start: 5299,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007f2,
        name_start: 5308,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007f3,
        name_start: 5319,
        name_len: 21,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007f4,
        name_start: 5340,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007f5,
        name_start: 5349,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007f6,
        name_start: 5362,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007f7,
        name_start: 5371,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007f8,
        name_start: 5380,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000007f9,
        name_start: 5389,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008a1,
        name_start: 5400,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008a2,
        name_start: 5411,
        name_len: 14,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008a3,
        name_start: 5425,
        name_len: 14,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008a4,
        name_start: 5439,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008a5,
        name_start: 5450,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008a6,
        name_start: 5461,
        name_len: 13,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008a7,
        name_start: 5474,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008a8,
        name_start: 5490,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008a9,
        name_start: 5506,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008aa,
        name_start: 5523,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008ab,
        name_start: 5540,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008ac,
        name_start: 5553,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008ad,
        name_start: 5566,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008ae,
        name_start: 5580,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008af,
        name_start: 5594,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008b0,
        name_start: 5614,
        name_len: 21,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008b1,
        name_start: 5635,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008b2,
        name_start: 5651,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008b3,
        name_start: 5667,
        name_len: 25,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008b4,
        name_start: 5692,
        name_len: 25,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008b5,
        name_start: 5717,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008b6,
        name_start: 5734,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008b7,
        name_start: 5751,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008bc,
        name_start: 5771,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008bd,
        name_start: 5784,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008be,
        name_start: 5792,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008bf,
        name_start: 5808,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008c0,
        name_start: 5816,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008c1,
        name_start: 5825,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008c2,
        name_start: 5834,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008c5,
        name_start: 5842,
        name_len: 5,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008c8,
        name_start: 5847,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008c9,
        name_start: 5858,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008cd,
        name_start: 5870,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008ce,
        name_start: 5878,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008cf,
        name_start: 5885,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008d6,
        name_start: 5894,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008da,
        name_start: 5901,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008db,
        name_start: 5911,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008dc,
        name_start: 5919,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008dd,
        name_start: 5931,
        name_len: 5,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008de,
        name_start: 5936,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008df,
        name_start: 5946,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008ef,
        name_start: 5955,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008f6,
        name_start: 5972,
        name_len: 8,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008fb,
        name_start: 5980,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008fc,
        name_start: 5989,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008fd,
        name_start: 5996,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008fe,
        name_start: 6006,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009df,
        name_start: 6015,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009e0,
        name_start: 6020,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009e1,
        name_start: 6032,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009e2,
        name_start: 6044,
        name_len: 2,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009e3,
        name_start: 6046,
        name_len: 2,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009e4,
        name_start: 6048,
        name_len: 2,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009e5,
        name_start: 6050,
        name_len: 2,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009e8,
        name_start: 6052,
        name_len: 2,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009e9,
        name_start: 6054,
        name_len: 2,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009ea,
        name_start: 6056,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009eb,
        name_start: 6070,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009ec,
        name_start: 6083,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009ed,
        name_start: 6095,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009ee,
        name_start: 6108,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009ef,
        name_start: 6121,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009f0,
        name_start: 6135,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009f1,
        name_start: 6149,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009f2,
        name_start: 6163,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009f3,
        name_start: 6177,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009f4,
        name_start: 6191,
        name_len: 5,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009f5,
        name_start: 6196,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009f6,
        name_start: 6202,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009f7,
        name_start: 6206,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009f8,
        name_start: 6210,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aa1,
        name_start: 6217,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aa2,
        name_start: 6224,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aa3,
        name_start: 6231,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aa4,
        name_start: 6239,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aa5,
        name_start: 6247,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aa6,
        name_start: 6257,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aa7,
        name_start: 6267,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aa8,
        name_start: 6276,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aa9,
        name_start: 6285,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aaa,
        name_start: 6291,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aac,
        name_start: 6297,
        name_len: 11,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aae,
        name_start: 6308,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aaf,
        name_start: 6316,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ab0,
        name_start: 6331,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ab1,
        name_start: 6339,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ab2,
        name_start: 6348,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ab3,
        name_start: 6356,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ab4,
        name_start: 6365,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ab5,
        name_start: 6376,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ab6,
        name_start: 6386,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ab7,
        name_start: 6394,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ab8,
        name_start: 6404,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000abb,
        name_start: 6410,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000abc,
        name_start: 6417,
        name_len: 16,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000abd,
        name_start: 6433,
        name_len: 12,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000abe,
        name_start: 6445,
        name_len: 17,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000abf,
        name_start: 6462,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ac3,
        name_start: 6468,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ac4,
        name_start: 6477,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ac5,
        name_start: 6489,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ac6,
        name_start: 6500,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ac9,
        name_start: 6512,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aca,
        name_start: 6521,
        name_len: 13,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000acb,
        name_start: 6534,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000acc,
        name_start: 6551,
        name_len: 16,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000acd,
        name_start: 6567,
        name_len: 17,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ace,
        name_start: 6584,
        name_len: 12,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000acf,
        name_start: 6596,
        name_len: 15,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ad0,
        name_start: 6611,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ad1,
        name_start: 6630,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ad2,
        name_start: 6650,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ad3,
        name_start: 6669,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ad4,
        name_start: 6689,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ad5,
        name_start: 6701,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ad6,
        name_start: 6709,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ad7,
        name_start: 6716,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ad9,
        name_start: 6723,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ada,
        name_start: 6733,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000adb,
        name_start: 6741,
        name_len: 16,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000adc,
        name_start: 6757,
        name_len: 19,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000add,
        name_start: 6776,
        name_len: 20,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ade,
        name_start: 6796,
        name_len: 14,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000adf,
        name_start: 6810,
        name_len: 12,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ae0,
        name_start: 6822,
        name_len: 16,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ae1,
        name_start: 6838,
        name_len: 18,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ae2,
        name_start: 6856,
        name_len: 14,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ae3,
        name_start: 6870,
        name_len: 15,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ae4,
        name_start: 6885,
        name_len: 17,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ae5,
        name_start: 6902,
        name_len: 8,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ae6,
        name_start: 6910,
        name_len: 18,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ae7,
        name_start: 6928,
        name_len: 16,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ae8,
        name_start: 6944,
        name_len: 17,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ae9,
        name_start: 6961,
        name_len: 19,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aea,
        name_start: 6980,
        name_len: 11,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aeb,
        name_start: 6991,
        name_len: 12,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aec,
        name_start: 7003,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aed,
        name_start: 7007,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aee,
        name_start: 7014,
        name_len: 5,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000af0,
        name_start: 7019,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000af1,
        name_start: 7031,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000af2,
        name_start: 7037,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000af3,
        name_start: 7049,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000af4,
        name_start: 7058,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000af5,
        name_start: 7069,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000af6,
        name_start: 7081,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000af7,
        name_start: 7092,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000af8,
        name_start: 7102,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000af9,
        name_start: 7114,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000afa,
        name_start: 7123,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000afb,
        name_start: 7140,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000afc,
        name_start: 7159,
        name_len: 5,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000afd,
        name_start: 7164,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000afe,
        name_start: 7182,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000aff,
        name_start: 7200,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ba3,
        name_start: 7206,
        name_len: 9,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ba6,
        name_start: 7215,
        name_len: 10,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ba8,
        name_start: 7225,
        name_len: 9,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ba9,
        name_start: 7234,
        name_len: 7,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bc0,
        name_start: 7241,
        name_len: 7,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bc2,
        name_start: 7248,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bc3,
        name_start: 7256,
        name_len: 6,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bc4,
        name_start: 7262,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bc6,
        name_start: 7271,
        name_len: 8,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bca,
        name_start: 7279,
        name_len: 3,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bcc,
        name_start: 7282,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bce,
        name_start: 7286,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bcf,
        name_start: 7292,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bd3,
        name_start: 7298,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bd6,
        name_start: 7305,
        name_len: 8,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bd8,
        name_start: 7313,
        name_len: 9,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bda,
        name_start: 7322,
        name_len: 8,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bdc,
        name_start: 7330,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000bfc,
        name_start: 7338,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cdf,
        name_start: 7347,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ce0,
        name_start: 7367,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ce1,
        name_start: 7379,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000031b,
        name_start: 7389,
        name_len: 11,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ce2,
        name_start: 7400,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000031d,
        name_start: 7412,
        name_len: 13,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ce3,
        name_start: 7425,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000031f,
        name_start: 7437,
        name_len: 13,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ce4,
        name_start: 7450,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ce5,
        name_start: 7459,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ce6,
        name_start: 7469,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000323,
        name_start: 7480,
        name_len: 12,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ce7,
        name_start: 7492,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000325,
        name_start: 7503,
        name_len: 10,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ce8,
        name_start: 7513,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000327,
        name_start: 7523,
        name_len: 11,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ce9,
        name_start: 7534,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cea,
        name_start: 7544,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ceb,
        name_start: 7560,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cec,
        name_start: 7571,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ced,
        name_start: 7583,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cee,
        name_start: 7598,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cef,
        name_start: 7608,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cf0,
        name_start: 7623,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cf1,
        name_start: 7633,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000331,
        name_start: 7646,
        name_len: 13,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cf2,
        name_start: 7659,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cf3,
        name_start: 7670,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cf4,
        name_start: 7684,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cf5,
        name_start: 7693,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000336,
        name_start: 7709,
        name_len: 16,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cf6,
        name_start: 7725,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000338,
        name_start: 7736,
        name_len: 11,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cf7,
        name_start: 7747,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000033a,
        name_start: 7758,
        name_len: 10,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cf8,
        name_start: 7768,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cf9,
        name_start: 7779,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000cfa,
        name_start: 7790,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000033e,
        name_start: 7800,
        name_len: 10,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000da1,
        name_start: 7810,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000da2,
        name_start: 7820,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000da3,
        name_start: 7832,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000da4,
        name_start: 7845,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000da5,
        name_start: 7858,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000da6,
        name_start: 7870,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000da7,
        name_start: 7885,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000da8,
        name_start: 7896,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000da9,
        name_start: 7908,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000daa,
        name_start: 7921,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dab,
        name_start: 7934,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dac,
        name_start: 7943,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dad,
        name_start: 7955,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dae,
        name_start: 7966,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000daf,
        name_start: 7978,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000db0,
        name_start: 7990,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000db1,
        name_start: 8002,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000db2,
        name_start: 8020,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000db3,
        name_start: 8035,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000db4,
        name_start: 8045,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000db5,
        name_start: 8055,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000db6,
        name_start: 8065,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000db7,
        name_start: 8078,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000db8,
        name_start: 8092,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000db9,
        name_start: 8105,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dba,
        name_start: 8114,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dbb,
        name_start: 8127,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dbc,
        name_start: 8137,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dbd,
        name_start: 8150,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dbe,
        name_start: 8159,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dbf,
        name_start: 8171,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dc0,
        name_start: 8181,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dc1,
        name_start: 8196,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dc2,
        name_start: 8205,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dc3,
        name_start: 8215,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dc4,
        name_start: 8225,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dc5,
        name_start: 8232,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dc6,
        name_start: 8243,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dc7,
        name_start: 8250,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dc8,
        name_start: 8261,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dc9,
        name_start: 8272,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dca,
        name_start: 8283,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dcb,
        name_start: 8293,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dcc,
        name_start: 8303,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dcd,
        name_start: 8315,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dce,
        name_start: 8324,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dcf,
        name_start: 8337,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dd0,
        name_start: 8351,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dd1,
        name_start: 8361,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dd2,
        name_start: 8376,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dd3,
        name_start: 8387,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dd4,
        name_start: 8398,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dd5,
        name_start: 8408,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dd6,
        name_start: 8419,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dd7,
        name_start: 8430,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dd8,
        name_start: 8442,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dd9,
        name_start: 8452,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dda,
        name_start: 8463,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dde,
        name_start: 8475,
        name_len: 22,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ddf,
        name_start: 8497,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000de0,
        name_start: 8506,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000de1,
        name_start: 8516,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000de2,
        name_start: 8527,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000de3,
        name_start: 8537,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000de4,
        name_start: 8555,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000de5,
        name_start: 8574,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000de6,
        name_start: 8590,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000de7,
        name_start: 8603,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000de8,
        name_start: 8617,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000de9,
        name_start: 8627,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dea,
        name_start: 8638,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000deb,
        name_start: 8649,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000dec,
        name_start: 8665,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ded,
        name_start: 8681,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000df0,
        name_start: 8694,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000df1,
        name_start: 8705,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000df2,
        name_start: 8717,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000df3,
        name_start: 8729,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000df4,
        name_start: 8740,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000df5,
        name_start: 8750,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000df6,
        name_start: 8760,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000df7,
        name_start: 8771,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000df8,
        name_start: 8783,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000df9,
        name_start: 8795,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ea1,
        name_start: 8806,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ea2,
        name_start: 8819,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ea3,
        name_start: 8837,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ea4,
        name_start: 8854,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ea5,
        name_start: 8866,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ea6,
        name_start: 8883,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ea7,
        name_start: 8900,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ea8,
        name_start: 8913,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ea9,
        name_start: 8931,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eaa,
        name_start: 8943,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eab,
        name_start: 8961,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eac,
        name_start: 8978,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ead,
        name_start: 8995,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eae,
        name_start: 9011,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eaf,
        name_start: 9028,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eb0,
        name_start: 9046,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eb1,
        name_start: 9063,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eb2,
        name_start: 9075,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eb3,
        name_start: 9087,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eb4,
        name_start: 9104,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eb5,
        name_start: 9120,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eb6,
        name_start: 9131,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eb7,
        name_start: 9147,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eb8,
        name_start: 9159,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eb9,
        name_start: 9171,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eba,
        name_start: 9188,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ebb,
        name_start: 9200,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ebc,
        name_start: 9213,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ebd,
        name_start: 9225,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ebe,
        name_start: 9238,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ebf,
        name_start: 9250,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ec0,
        name_start: 9258,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ec1,
        name_start: 9267,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ec2,
        name_start: 9276,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ec3,
        name_start: 9286,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ec4,
        name_start: 9295,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ec5,
        name_start: 9303,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ec6,
        name_start: 9313,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ec7,
        name_start: 9322,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ec8,
        name_start: 9330,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ec9,
        name_start: 9339,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eca,
        name_start: 9349,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ecb,
        name_start: 9358,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ecc,
        name_start: 9367,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ecd,
        name_start: 9375,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ece,
        name_start: 9385,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ecf,
        name_start: 9394,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ed0,
        name_start: 9403,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ed1,
        name_start: 9412,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ed2,
        name_start: 9421,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ed3,
        name_start: 9430,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ed4,
        name_start: 9438,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ed5,
        name_start: 9453,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ed6,
        name_start: 9473,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ed7,
        name_start: 9492,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ed8,
        name_start: 9506,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ed9,
        name_start: 9525,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eda,
        name_start: 9544,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000edb,
        name_start: 9559,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000edc,
        name_start: 9573,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000edd,
        name_start: 9593,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ede,
        name_start: 9612,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000edf,
        name_start: 9631,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ee0,
        name_start: 9649,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ee1,
        name_start: 9668,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ee2,
        name_start: 9688,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ee3,
        name_start: 9707,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ee4,
        name_start: 9721,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ee5,
        name_start: 9735,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ee6,
        name_start: 9753,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ee7,
        name_start: 9766,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ee8,
        name_start: 9784,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ee9,
        name_start: 9798,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eea,
        name_start: 9812,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eeb,
        name_start: 9826,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eec,
        name_start: 9841,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eed,
        name_start: 9855,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eee,
        name_start: 9870,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eef,
        name_start: 9884,
        name_len: 23,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ef0,
        name_start: 9907,
        name_len: 24,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ef1,
        name_start: 9931,
        name_len: 24,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ef2,
        name_start: 9955,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ef3,
        name_start: 9969,
        name_len: 24,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ef4,
        name_start: 9993,
        name_len: 25,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ef5,
        name_start: 10018,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ef6,
        name_start: 10036,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ef7,
        name_start: 10048,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ef8,
        name_start: 10061,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000ef9,
        name_start: 10077,
        name_len: 26,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000efa,
        name_start: 10103,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000eff,
        name_start: 10123,
        name_len: 10,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000013bc,
        name_start: 10133,
        name_len: 2,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000013bd,
        name_start: 10135,
        name_len: 2,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000013be,
        name_start: 10137,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000020ac,
        name_start: 10147,
        name_len: 8,
        flags: 0 | HAS_CHAR | KEYSYM_IS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd01,
        name_start: 10155,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd02,
        name_start: 10169,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd03,
        name_start: 10183,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd04,
        name_start: 10194,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd05,
        name_start: 10204,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd06,
        name_start: 10216,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd07,
        name_start: 10229,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd08,
        name_start: 10244,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd09,
        name_start: 10254,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd0a,
        name_start: 10263,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd0b,
        name_start: 10271,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd0c,
        name_start: 10279,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd0d,
        name_start: 10287,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd0e,
        name_start: 10296,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd0f,
        name_start: 10305,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd10,
        name_start: 10321,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd11,
        name_start: 10335,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd12,
        name_start: 10348,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd13,
        name_start: 10357,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd14,
        name_start: 10367,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd15,
        name_start: 10376,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd16,
        name_start: 10385,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd17,
        name_start: 10394,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd18,
        name_start: 10404,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd19,
        name_start: 10415,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd1a,
        name_start: 10432,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd1b,
        name_start: 10447,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd1c,
        name_start: 10460,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd1d,
        name_start: 10477,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fd1e,
        name_start: 10493,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe01,
        name_start: 10503,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe02,
        name_start: 10511,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe03,
        name_start: 10527,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe04,
        name_start: 10543,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe05,
        name_start: 10559,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe06,
        name_start: 10574,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe07,
        name_start: 10589,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe08,
        name_start: 10603,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe09,
        name_start: 10617,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe0a,
        name_start: 10636,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe0b,
        name_start: 10650,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe0c,
        name_start: 10669,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe0d,
        name_start: 10684,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe0e,
        name_start: 10704,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe0f,
        name_start: 10718,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe11,
        name_start: 10737,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe12,
        name_start: 10753,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe13,
        name_start: 10769,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe20,
        name_start: 10784,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe21,
        name_start: 10796,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe22,
        name_start: 10812,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe23,
        name_start: 10830,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe24,
        name_start: 10849,
        name_len: 21,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe25,
        name_start: 10870,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe26,
        name_start: 10892,
        name_len: 23,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe27,
        name_start: 10915,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe28,
        name_start: 10934,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe29,
        name_start: 10954,
        name_len: 23,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe2a,
        name_start: 10977,
        name_len: 24,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe2b,
        name_start: 11001,
        name_len: 24,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe2c,
        name_start: 11025,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe2d,
        name_start: 11045,
        name_len: 21,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe2e,
        name_start: 11066,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe2f,
        name_start: 11084,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe30,
        name_start: 11104,
        name_len: 24,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe31,
        name_start: 11128,
        name_len: 27,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe32,
        name_start: 11155,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe33,
        name_start: 11168,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe34,
        name_start: 11185,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe50,
        name_start: 11194,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe51,
        name_start: 11204,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe52,
        name_start: 11214,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe53,
        name_start: 11229,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000043b,
        name_start: 11239,
        name_len: 16,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe54,
        name_start: 11255,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe55,
        name_start: 11266,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe56,
        name_start: 11276,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe57,
        name_start: 11289,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe58,
        name_start: 11303,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe59,
        name_start: 11317,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe5a,
        name_start: 11333,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe5b,
        name_start: 11343,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe5c,
        name_start: 11355,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe5d,
        name_start: 11366,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe5e,
        name_start: 11375,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe5f,
        name_start: 11392,
        name_len: 21,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe60,
        name_start: 11413,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe61,
        name_start: 11426,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe62,
        name_start: 11435,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe63,
        name_start: 11444,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe64,
        name_start: 11455,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000044d,
        name_start: 11470,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe65,
        name_start: 11480,
        name_len: 23,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000044f,
        name_start: 11503,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe66,
        name_start: 11513,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe67,
        name_start: 11529,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe68,
        name_start: 11543,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe69,
        name_start: 11559,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe6a,
        name_start: 11579,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe6b,
        name_start: 11594,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe6c,
        name_start: 11609,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe6d,
        name_start: 11628,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe6e,
        name_start: 11646,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe6f,
        name_start: 11661,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe70,
        name_start: 11674,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe71,
        name_start: 11688,
        name_len: 23,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe72,
        name_start: 11711,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe73,
        name_start: 11728,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe74,
        name_start: 11743,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe75,
        name_start: 11760,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe76,
        name_start: 11777,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe77,
        name_start: 11793,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe78,
        name_start: 11815,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe79,
        name_start: 11830,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe7a,
        name_start: 11845,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe80,
        name_start: 11863,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe81,
        name_start: 11869,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe82,
        name_start: 11875,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe83,
        name_start: 11881,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe84,
        name_start: 11887,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe85,
        name_start: 11893,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe86,
        name_start: 11899,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe87,
        name_start: 11905,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe88,
        name_start: 11911,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe89,
        name_start: 11917,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe8a,
        name_start: 11923,
        name_len: 16,
        flags: 0 | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000470,
        name_start: 11939,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe8b,
        name_start: 11949,
        name_len: 18,
        flags: 0 | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000472,
        name_start: 11967,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe8c,
        name_start: 11977,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe8d,
        name_start: 11987,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe8e,
        name_start: 11997,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe90,
        name_start: 12012,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe91,
        name_start: 12024,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe92,
        name_start: 12046,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fe93,
        name_start: 12068,
        name_len: 23,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fea0,
        name_start: 12091,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fea1,
        name_start: 12093,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fea2,
        name_start: 12095,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fea3,
        name_start: 12097,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fea4,
        name_start: 12100,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fea5,
        name_start: 12103,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fed0,
        name_start: 12106,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fed1,
        name_start: 12126,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fed2,
        name_start: 12145,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fed4,
        name_start: 12164,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fed5,
        name_start: 12183,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fee0,
        name_start: 12199,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fee1,
        name_start: 12211,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fee2,
        name_start: 12224,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fee3,
        name_start: 12234,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fee4,
        name_start: 12246,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fee5,
        name_start: 12260,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fee6,
        name_start: 12275,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fee7,
        name_start: 12291,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fee8,
        name_start: 12308,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fee9,
        name_start: 12327,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000feea,
        name_start: 12342,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000feeb,
        name_start: 12357,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000feec,
        name_start: 12372,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000feed,
        name_start: 12387,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000feee,
        name_start: 12402,
        name_len: 21,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000feef,
        name_start: 12423,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fef0,
        name_start: 12440,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fef1,
        name_start: 12457,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fef2,
        name_start: 12474,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fef3,
        name_start: 12491,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fef4,
        name_start: 12508,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fef5,
        name_start: 12525,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fef6,
        name_start: 12538,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fef7,
        name_start: 12551,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fef8,
        name_start: 12564,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fef9,
        name_start: 12577,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fefa,
        name_start: 12595,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fefb,
        name_start: 12613,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fefc,
        name_start: 12632,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fefd,
        name_start: 12651,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff08,
        name_start: 12664,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff09,
        name_start: 12673,
        name_len: 3,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff0a,
        name_start: 12676,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff0b,
        name_start: 12684,
        name_len: 5,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff0d,
        name_start: 12689,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff13,
        name_start: 12695,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff14,
        name_start: 12700,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff15,
        name_start: 12711,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff1b,
        name_start: 12718,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff20,
        name_start: 12724,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004ad,
        name_start: 12733,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff21,
        name_start: 12743,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff22,
        name_start: 12748,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff23,
        name_start: 12756,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004b1,
        name_start: 12767,
        name_len: 6,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff24,
        name_start: 12773,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff25,
        name_start: 12779,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff26,
        name_start: 12787,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff27,
        name_start: 12795,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff28,
        name_start: 12812,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff29,
        name_start: 12819,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff2a,
        name_start: 12826,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff2b,
        name_start: 12841,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff2c,
        name_start: 12848,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff2d,
        name_start: 12854,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff2e,
        name_start: 12863,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff2f,
        name_start: 12873,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff30,
        name_start: 12883,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff31,
        name_start: 12894,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff32,
        name_start: 12900,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff33,
        name_start: 12912,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff34,
        name_start: 12922,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff35,
        name_start: 12934,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff36,
        name_start: 12945,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff37,
        name_start: 12958,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c6,
        name_start: 12967,
        name_len: 12,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004c6,
        name_start: 12979,
        name_len: 16,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff38,
        name_start: 12995,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff39,
        name_start: 13008,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff3a,
        name_start: 13020,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff3b,
        name_start: 13035,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff3c,
        name_start: 13051,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004cd,
        name_start: 13066,
        name_len: 22,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff3d,
        name_start: 13088,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004cf,
        name_start: 13105,
        name_len: 8,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004cf,
        name_start: 13113,
        name_len: 24,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff3e,
        name_start: 13137,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d2,
        name_start: 13154,
        name_len: 8,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004d2,
        name_start: 13162,
        name_len: 24,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff3f,
        name_start: 13186,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff50,
        name_start: 13200,
        name_len: 4,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff51,
        name_start: 13204,
        name_len: 4,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff52,
        name_start: 13208,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff53,
        name_start: 13210,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff54,
        name_start: 13215,
        name_len: 4,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff55,
        name_start: 13219,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004db,
        name_start: 13224,
        name_len: 7,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004db,
        name_start: 13231,
        name_len: 9,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff56,
        name_start: 13240,
        name_len: 4,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004de,
        name_start: 13244,
        name_len: 9,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004de,
        name_start: 13253,
        name_len: 11,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff57,
        name_start: 13264,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff58,
        name_start: 13267,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff60,
        name_start: 13272,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff61,
        name_start: 13278,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004e4,
        name_start: 13283,
        name_len: 15,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff62,
        name_start: 13298,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff63,
        name_start: 13305,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff65,
        name_start: 13311,
        name_len: 4,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004e8,
        name_start: 13315,
        name_len: 7,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff66,
        name_start: 13322,
        name_len: 4,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004ea,
        name_start: 13326,
        name_len: 8,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff67,
        name_start: 13334,
        name_len: 4,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff68,
        name_start: 13338,
        name_len: 4,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004ed,
        name_start: 13342,
        name_len: 7,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff69,
        name_start: 13349,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004ef,
        name_start: 13355,
        name_len: 7,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff6a,
        name_start: 13362,
        name_len: 4,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff6b,
        name_start: 13366,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff7e,
        name_start: 13371,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004f3,
        name_start: 13386,
        name_len: 11,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004f3,
        name_start: 13397,
        name_len: 13,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004f3,
        name_start: 13410,
        name_len: 11,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004f3,
        name_start: 13421,
        name_len: 13,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004f3,
        name_start: 13434,
        name_len: 12,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004f3,
        name_start: 13446,
        name_len: 13,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004f3,
        name_start: 13459,
        name_len: 13,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000004f3,
        name_start: 13472,
        name_len: 11,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff7f,
        name_start: 13483,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff80,
        name_start: 13491,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff89,
        name_start: 13499,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff8d,
        name_start: 13505,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff91,
        name_start: 13513,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff92,
        name_start: 13518,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff93,
        name_start: 13523,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff94,
        name_start: 13528,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff95,
        name_start: 13533,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff96,
        name_start: 13540,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff97,
        name_start: 13547,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff98,
        name_start: 13552,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff99,
        name_start: 13560,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff9a,
        name_start: 13567,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000509,
        name_start: 13575,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff9b,
        name_start: 13585,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000050b,
        name_start: 13592,
        name_len: 12,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff9c,
        name_start: 13604,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff9d,
        name_start: 13610,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff9e,
        name_start: 13618,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ff9f,
        name_start: 13627,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffaa,
        name_start: 13636,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffab,
        name_start: 13647,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffac,
        name_start: 13653,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffad,
        name_start: 13665,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffae,
        name_start: 13676,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffaf,
        name_start: 13686,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffb0,
        name_start: 13695,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffb1,
        name_start: 13699,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffb2,
        name_start: 13703,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffb3,
        name_start: 13707,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffb4,
        name_start: 13711,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffb5,
        name_start: 13715,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffb6,
        name_start: 13719,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffb7,
        name_start: 13723,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffb8,
        name_start: 13727,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffb9,
        name_start: 13731,
        name_len: 4,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffbd,
        name_start: 13735,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffbe,
        name_start: 13743,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffbf,
        name_start: 13745,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffc0,
        name_start: 13747,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffc1,
        name_start: 13749,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffc2,
        name_start: 13751,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffc3,
        name_start: 13753,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffc4,
        name_start: 13755,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffc5,
        name_start: 13757,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffc6,
        name_start: 13759,
        name_len: 2,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffc7,
        name_start: 13761,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffc8,
        name_start: 13764,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000052c,
        name_start: 13767,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffc9,
        name_start: 13769,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000052e,
        name_start: 13772,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffca,
        name_start: 13774,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000530,
        name_start: 13777,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffcb,
        name_start: 13779,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000532,
        name_start: 13782,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffcc,
        name_start: 13784,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000534,
        name_start: 13787,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffcd,
        name_start: 13789,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000536,
        name_start: 13792,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffce,
        name_start: 13794,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000538,
        name_start: 13797,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffcf,
        name_start: 13799,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000053a,
        name_start: 13802,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffd0,
        name_start: 13804,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000053c,
        name_start: 13807,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffd1,
        name_start: 13809,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000053e,
        name_start: 13812,
        name_len: 3,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffd2,
        name_start: 13815,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000540,
        name_start: 13818,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffd3,
        name_start: 13820,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000542,
        name_start: 13823,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffd4,
        name_start: 13825,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000544,
        name_start: 13828,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffd5,
        name_start: 13830,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000546,
        name_start: 13833,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffd6,
        name_start: 13835,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000548,
        name_start: 13838,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffd7,
        name_start: 13840,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000054a,
        name_start: 13843,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffd8,
        name_start: 13845,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000054c,
        name_start: 13848,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffd9,
        name_start: 13850,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000054e,
        name_start: 13853,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffda,
        name_start: 13855,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000550,
        name_start: 13858,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffdb,
        name_start: 13860,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000552,
        name_start: 13863,
        name_len: 3,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffdc,
        name_start: 13866,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000554,
        name_start: 13869,
        name_len: 3,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffdd,
        name_start: 13872,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000556,
        name_start: 13875,
        name_len: 3,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffde,
        name_start: 13878,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000558,
        name_start: 13881,
        name_len: 3,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffdf,
        name_start: 13884,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000055a,
        name_start: 13887,
        name_len: 3,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffe0,
        name_start: 13890,
        name_len: 3,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000055c,
        name_start: 13893,
        name_len: 3,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffe1,
        name_start: 13896,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffe2,
        name_start: 13903,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffe3,
        name_start: 13910,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffe4,
        name_start: 13919,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffe5,
        name_start: 13928,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffe6,
        name_start: 13937,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffe7,
        name_start: 13947,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffe8,
        name_start: 13953,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffe9,
        name_start: 13959,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffea,
        name_start: 13964,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffeb,
        name_start: 13969,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffec,
        name_start: 13976,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffed,
        name_start: 13983,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffee,
        name_start: 13990,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fff1,
        name_start: 13997,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fff2,
        name_start: 14010,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fff3,
        name_start: 14023,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fff4,
        name_start: 14036,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fff5,
        name_start: 14049,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fff6,
        name_start: 14062,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fff7,
        name_start: 14075,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fff8,
        name_start: 14088,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fff9,
        name_start: 14101,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000fffa,
        name_start: 14114,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000ffff,
        name_start: 14128,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00ffffff,
        name_start: 14134,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100012c,
        name_start: 14144,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100012d,
        name_start: 14150,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000174,
        name_start: 14156,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000175,
        name_start: 14167,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000176,
        name_start: 14178,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000177,
        name_start: 14189,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100018f,
        name_start: 14200,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100019f,
        name_start: 14205,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010001a0,
        name_start: 14212,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010001a1,
        name_start: 14217,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010001af,
        name_start: 14222,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010001b0,
        name_start: 14227,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010001b5,
        name_start: 14232,
        name_len: 7,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010001b6,
        name_start: 14239,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010001b7,
        name_start: 14246,
        name_len: 3,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010001d1,
        name_start: 14249,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010001d2,
        name_start: 14255,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010001e6,
        name_start: 14261,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010001e7,
        name_start: 14267,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000259,
        name_start: 14273,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000275,
        name_start: 14278,
        name_len: 7,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000292,
        name_start: 14285,
        name_len: 3,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000300,
        name_start: 14288,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000301,
        name_start: 14303,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000303,
        name_start: 14318,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000309,
        name_start: 14333,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000323,
        name_start: 14347,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000492,
        name_start: 14365,
        name_len: 16,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000493,
        name_start: 14381,
        name_len: 16,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000496,
        name_start: 14397,
        name_len: 22,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000497,
        name_start: 14419,
        name_len: 22,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100049a,
        name_start: 14441,
        name_len: 21,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100049b,
        name_start: 14462,
        name_len: 21,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100049c,
        name_start: 14483,
        name_len: 22,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100049d,
        name_start: 14505,
        name_len: 22,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004a2,
        name_start: 14527,
        name_len: 21,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004a3,
        name_start: 14548,
        name_len: 21,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004ae,
        name_start: 14569,
        name_len: 19,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004af,
        name_start: 14588,
        name_len: 19,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004b0,
        name_start: 14607,
        name_len: 23,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004b1,
        name_start: 14630,
        name_len: 23,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004b2,
        name_start: 14653,
        name_len: 21,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004b3,
        name_start: 14674,
        name_len: 21,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004b6,
        name_start: 14695,
        name_len: 22,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004b7,
        name_start: 14717,
        name_len: 22,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004b8,
        name_start: 14739,
        name_len: 23,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004b9,
        name_start: 14762,
        name_len: 23,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004ba,
        name_start: 14785,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004bb,
        name_start: 14798,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004d8,
        name_start: 14811,
        name_len: 14,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004d9,
        name_start: 14825,
        name_len: 14,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004e2,
        name_start: 14839,
        name_len: 17,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004e3,
        name_start: 14856,
        name_len: 17,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004e8,
        name_start: 14873,
        name_len: 14,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004e9,
        name_start: 14887,
        name_len: 14,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004ee,
        name_start: 14901,
        name_len: 17,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010004ef,
        name_start: 14918,
        name_len: 17,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000531,
        name_start: 14935,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000532,
        name_start: 14947,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000533,
        name_start: 14959,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000534,
        name_start: 14971,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000535,
        name_start: 14982,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000536,
        name_start: 14995,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000537,
        name_start: 15006,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000538,
        name_start: 15016,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000539,
        name_start: 15027,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100053a,
        name_start: 15038,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100053b,
        name_start: 15050,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100053c,
        name_start: 15062,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100053d,
        name_start: 15075,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100053e,
        name_start: 15087,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100053f,
        name_start: 15099,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000540,
        name_start: 15111,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000541,
        name_start: 15122,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000542,
        name_start: 15134,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000543,
        name_start: 15147,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000544,
        name_start: 15160,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000545,
        name_start: 15172,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000546,
        name_start: 15183,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000547,
        name_start: 15194,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000548,
        name_start: 15206,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000549,
        name_start: 15217,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100054a,
        name_start: 15229,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100054b,
        name_start: 15240,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100054c,
        name_start: 15251,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100054d,
        name_start: 15262,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100054e,
        name_start: 15273,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100054f,
        name_start: 15285,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000550,
        name_start: 15298,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000551,
        name_start: 15309,
        name_len: 12,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000552,
        name_start: 15321,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000553,
        name_start: 15334,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000554,
        name_start: 15347,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000555,
        name_start: 15358,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000556,
        name_start: 15368,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100055a,
        name_start: 15379,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100055b,
        name_start: 15398,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005d8,
        name_start: 15413,
        name_len: 15,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100055c,
        name_start: 15428,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005da,
        name_start: 15443,
        name_len: 15,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100055d,
        name_start: 15458,
        name_len: 24,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005dc,
        name_start: 15482,
        name_len: 12,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100055e,
        name_start: 15494,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000005de,
        name_start: 15511,
        name_len: 15,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000561,
        name_start: 15526,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000562,
        name_start: 15538,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000563,
        name_start: 15550,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000564,
        name_start: 15562,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000565,
        name_start: 15573,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000566,
        name_start: 15586,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000567,
        name_start: 15597,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000568,
        name_start: 15607,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000569,
        name_start: 15618,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100056a,
        name_start: 15629,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100056b,
        name_start: 15641,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100056c,
        name_start: 15653,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100056d,
        name_start: 15666,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100056e,
        name_start: 15678,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100056f,
        name_start: 15690,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000570,
        name_start: 15702,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000571,
        name_start: 15713,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000572,
        name_start: 15725,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000573,
        name_start: 15738,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000574,
        name_start: 15751,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000575,
        name_start: 15763,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000576,
        name_start: 15774,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000577,
        name_start: 15785,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000578,
        name_start: 15797,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000579,
        name_start: 15808,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100057a,
        name_start: 15820,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100057b,
        name_start: 15831,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100057c,
        name_start: 15842,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100057d,
        name_start: 15853,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100057e,
        name_start: 15864,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100057f,
        name_start: 15876,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000580,
        name_start: 15889,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000581,
        name_start: 15900,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000582,
        name_start: 15912,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000583,
        name_start: 15925,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000584,
        name_start: 15938,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000585,
        name_start: 15949,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000586,
        name_start: 15959,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000587,
        name_start: 15970,
        name_len: 20,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000589,
        name_start: 15990,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000607,
        name_start: 16008,
        name_len: 17,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100058a,
        name_start: 16025,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000609,
        name_start: 16040,
        name_len: 17,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000653,
        name_start: 16057,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000654,
        name_start: 16075,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000655,
        name_start: 16093,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000660,
        name_start: 16111,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000661,
        name_start: 16119,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000662,
        name_start: 16127,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000663,
        name_start: 16135,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000664,
        name_start: 16143,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000665,
        name_start: 16151,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000666,
        name_start: 16159,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000667,
        name_start: 16167,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000668,
        name_start: 16175,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000669,
        name_start: 16183,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100066a,
        name_start: 16191,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000670,
        name_start: 16205,
        name_len: 23,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000679,
        name_start: 16228,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100067e,
        name_start: 16239,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000686,
        name_start: 16249,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000688,
        name_start: 16261,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000691,
        name_start: 16272,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000698,
        name_start: 16283,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006a4,
        name_start: 16293,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006a9,
        name_start: 16303,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006af,
        name_start: 16315,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006ba,
        name_start: 16325,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006be,
        name_start: 16343,
        name_len: 22,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006c1,
        name_start: 16365,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006cc,
        name_start: 16380,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000626,
        name_start: 16389,
        name_len: 16,
        flags: 0 | HAS_CHAR | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006d2,
        name_start: 16405,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006d4,
        name_start: 16421,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006f0,
        name_start: 16436,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006f1,
        name_start: 16443,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006f2,
        name_start: 16450,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006f3,
        name_start: 16457,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006f4,
        name_start: 16464,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006f5,
        name_start: 16471,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006f6,
        name_start: 16478,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006f7,
        name_start: 16485,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006f8,
        name_start: 16492,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010006f9,
        name_start: 16499,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d82,
        name_start: 16506,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d83,
        name_start: 16513,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d85,
        name_start: 16520,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d86,
        name_start: 16526,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d87,
        name_start: 16533,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d88,
        name_start: 16540,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d89,
        name_start: 16548,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d8a,
        name_start: 16554,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d8b,
        name_start: 16561,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d8c,
        name_start: 16567,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d8d,
        name_start: 16574,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d8e,
        name_start: 16581,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d8f,
        name_start: 16589,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d90,
        name_start: 16596,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d91,
        name_start: 16604,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d92,
        name_start: 16610,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d93,
        name_start: 16617,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d94,
        name_start: 16624,
        name_len: 6,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d95,
        name_start: 16630,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d96,
        name_start: 16637,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d9a,
        name_start: 16644,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d9b,
        name_start: 16651,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d9c,
        name_start: 16659,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d9d,
        name_start: 16666,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d9e,
        name_start: 16674,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000d9f,
        name_start: 16682,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000da0,
        name_start: 16690,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000da1,
        name_start: 16697,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000da2,
        name_start: 16705,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000da3,
        name_start: 16712,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000da4,
        name_start: 16720,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000da5,
        name_start: 16728,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000da6,
        name_start: 16737,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000da7,
        name_start: 16745,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000da8,
        name_start: 16753,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000da9,
        name_start: 16762,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000daa,
        name_start: 16770,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dab,
        name_start: 16779,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dac,
        name_start: 16787,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dad,
        name_start: 16796,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dae,
        name_start: 16804,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000daf,
        name_start: 16813,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000db0,
        name_start: 16821,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000db1,
        name_start: 16830,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000db3,
        name_start: 16837,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000db4,
        name_start: 16846,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000db5,
        name_start: 16853,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000db6,
        name_start: 16861,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000db7,
        name_start: 16868,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000db8,
        name_start: 16876,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000db9,
        name_start: 16883,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dba,
        name_start: 16891,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dbb,
        name_start: 16898,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dbd,
        name_start: 16905,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dc0,
        name_start: 16912,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dc1,
        name_start: 16919,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dc2,
        name_start: 16927,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dc3,
        name_start: 16936,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dc4,
        name_start: 16943,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dc5,
        name_start: 16950,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dc6,
        name_start: 16958,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dca,
        name_start: 16965,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dcf,
        name_start: 16972,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dd0,
        name_start: 16980,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dd1,
        name_start: 16988,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dd2,
        name_start: 16997,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dd3,
        name_start: 17004,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dd4,
        name_start: 17012,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dd6,
        name_start: 17019,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dd8,
        name_start: 17027,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dd9,
        name_start: 17035,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dda,
        name_start: 17042,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000ddb,
        name_start: 17050,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000ddc,
        name_start: 17058,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000ddd,
        name_start: 17065,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000dde,
        name_start: 17073,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000ddf,
        name_start: 17081,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000df2,
        name_start: 17089,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000df3,
        name_start: 17098,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01000df4,
        name_start: 17107,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010d0,
        name_start: 17122,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010d1,
        name_start: 17133,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010d2,
        name_start: 17145,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010d3,
        name_start: 17157,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010d4,
        name_start: 17169,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010d5,
        name_start: 17180,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010d6,
        name_start: 17192,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010d7,
        name_start: 17204,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010d8,
        name_start: 17216,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010d9,
        name_start: 17227,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010da,
        name_start: 17239,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010db,
        name_start: 17251,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010dc,
        name_start: 17263,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010dd,
        name_start: 17275,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010de,
        name_start: 17286,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010df,
        name_start: 17298,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010e0,
        name_start: 17311,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010e1,
        name_start: 17323,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010e2,
        name_start: 17335,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010e3,
        name_start: 17347,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010e4,
        name_start: 17358,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010e5,
        name_start: 17371,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010e6,
        name_start: 17384,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010e7,
        name_start: 17397,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010e8,
        name_start: 17409,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010e9,
        name_start: 17422,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010ea,
        name_start: 17435,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010eb,
        name_start: 17447,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010ec,
        name_start: 17459,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010ed,
        name_start: 17471,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010ee,
        name_start: 17484,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010ef,
        name_start: 17496,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010f0,
        name_start: 17509,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010f1,
        name_start: 17521,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010f2,
        name_start: 17532,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010f3,
        name_start: 17544,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010f4,
        name_start: 17555,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010f5,
        name_start: 17567,
        name_len: 12,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010010f6,
        name_start: 17579,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e02,
        name_start: 17590,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e03,
        name_start: 17599,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e0a,
        name_start: 17608,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e0b,
        name_start: 17617,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e1e,
        name_start: 17626,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e1f,
        name_start: 17635,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e36,
        name_start: 17644,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e37,
        name_start: 17653,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e40,
        name_start: 17662,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e41,
        name_start: 17671,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e56,
        name_start: 17680,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e57,
        name_start: 17689,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e60,
        name_start: 17698,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e61,
        name_start: 17707,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e6a,
        name_start: 17716,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e6b,
        name_start: 17725,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e80,
        name_start: 17734,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e81,
        name_start: 17740,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e82,
        name_start: 17746,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e83,
        name_start: 17752,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e84,
        name_start: 17758,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e85,
        name_start: 17768,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e8a,
        name_start: 17778,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e8b,
        name_start: 17787,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001e9e,
        name_start: 17796,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ea0,
        name_start: 17802,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ea1,
        name_start: 17811,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ea2,
        name_start: 17820,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ea3,
        name_start: 17825,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ea4,
        name_start: 17830,
        name_len: 16,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ea5,
        name_start: 17846,
        name_len: 16,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ea6,
        name_start: 17862,
        name_len: 16,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ea7,
        name_start: 17878,
        name_len: 16,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ea8,
        name_start: 17894,
        name_len: 15,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ea9,
        name_start: 17909,
        name_len: 15,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eaa,
        name_start: 17924,
        name_len: 16,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eab,
        name_start: 17940,
        name_len: 16,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eac,
        name_start: 17956,
        name_len: 19,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ead,
        name_start: 17975,
        name_len: 19,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eae,
        name_start: 17994,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eaf,
        name_start: 18005,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eb0,
        name_start: 18016,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eb1,
        name_start: 18027,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eb2,
        name_start: 18038,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eb3,
        name_start: 18048,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eb4,
        name_start: 18058,
        name_len: 11,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eb5,
        name_start: 18069,
        name_len: 11,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eb6,
        name_start: 18080,
        name_len: 14,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eb7,
        name_start: 18094,
        name_len: 14,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eb8,
        name_start: 18108,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eb9,
        name_start: 18117,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eba,
        name_start: 18126,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ebb,
        name_start: 18131,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ebc,
        name_start: 18136,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ebd,
        name_start: 18142,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ebe,
        name_start: 18148,
        name_len: 16,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ebf,
        name_start: 18164,
        name_len: 16,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ec0,
        name_start: 18180,
        name_len: 16,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ec1,
        name_start: 18196,
        name_len: 16,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ec2,
        name_start: 18212,
        name_len: 15,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ec3,
        name_start: 18227,
        name_len: 15,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ec4,
        name_start: 18242,
        name_len: 16,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ec5,
        name_start: 18258,
        name_len: 16,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ec6,
        name_start: 18274,
        name_len: 19,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ec7,
        name_start: 18293,
        name_len: 19,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ec8,
        name_start: 18312,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ec9,
        name_start: 18317,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eca,
        name_start: 18322,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ecb,
        name_start: 18331,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ecc,
        name_start: 18340,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ecd,
        name_start: 18349,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ece,
        name_start: 18358,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ecf,
        name_start: 18363,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ed0,
        name_start: 18368,
        name_len: 16,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ed1,
        name_start: 18384,
        name_len: 16,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ed2,
        name_start: 18400,
        name_len: 16,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ed3,
        name_start: 18416,
        name_len: 16,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ed4,
        name_start: 18432,
        name_len: 15,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ed5,
        name_start: 18447,
        name_len: 15,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ed6,
        name_start: 18462,
        name_len: 16,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ed7,
        name_start: 18478,
        name_len: 16,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ed8,
        name_start: 18494,
        name_len: 19,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ed9,
        name_start: 18513,
        name_len: 19,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eda,
        name_start: 18532,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001edb,
        name_start: 18542,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001edc,
        name_start: 18552,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001edd,
        name_start: 18562,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ede,
        name_start: 18572,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001edf,
        name_start: 18581,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ee0,
        name_start: 18590,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ee1,
        name_start: 18600,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ee2,
        name_start: 18610,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ee3,
        name_start: 18623,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ee4,
        name_start: 18636,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ee5,
        name_start: 18645,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ee6,
        name_start: 18654,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ee7,
        name_start: 18659,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ee8,
        name_start: 18664,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ee9,
        name_start: 18674,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eea,
        name_start: 18684,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eeb,
        name_start: 18694,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eec,
        name_start: 18704,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eed,
        name_start: 18713,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eee,
        name_start: 18722,
        name_len: 10,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001eef,
        name_start: 18732,
        name_len: 10,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ef0,
        name_start: 18742,
        name_len: 13,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ef1,
        name_start: 18755,
        name_len: 13,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ef2,
        name_start: 18768,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ef3,
        name_start: 18774,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ef4,
        name_start: 18780,
        name_len: 9,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ef5,
        name_start: 18789,
        name_len: 9,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ef6,
        name_start: 18798,
        name_len: 5,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ef7,
        name_start: 18803,
        name_len: 5,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ef8,
        name_start: 18808,
        name_len: 6,
        flags: 0 | IS_UPPER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01001ef9,
        name_start: 18814,
        name_len: 6,
        flags: 0 | IS_LOWER | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002039,
        name_start: 18820,
        name_len: 24,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100203a,
        name_start: 18844,
        name_len: 25,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002070,
        name_start: 18869,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002074,
        name_start: 18881,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002075,
        name_start: 18893,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002076,
        name_start: 18905,
        name_len: 11,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002077,
        name_start: 18916,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002078,
        name_start: 18929,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002079,
        name_start: 18942,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002080,
        name_start: 18954,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002081,
        name_start: 18967,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002082,
        name_start: 18979,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002083,
        name_start: 18991,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002084,
        name_start: 19005,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002085,
        name_start: 19018,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002086,
        name_start: 19031,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002087,
        name_start: 19043,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002088,
        name_start: 19057,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002089,
        name_start: 19071,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020a0,
        name_start: 19084,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020a1,
        name_start: 19091,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020a2,
        name_start: 19100,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020a3,
        name_start: 19112,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020a4,
        name_start: 19122,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020a5,
        name_start: 19130,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020a6,
        name_start: 19138,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020a7,
        name_start: 19147,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020a8,
        name_start: 19157,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020a9,
        name_start: 19166,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020aa,
        name_start: 19173,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010020ab,
        name_start: 19186,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002202,
        name_start: 19194,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002205,
        name_start: 19210,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002208,
        name_start: 19218,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002209,
        name_start: 19227,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100220b,
        name_start: 19239,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100221a,
        name_start: 19249,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100221b,
        name_start: 19259,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100221c,
        name_start: 19267,
        name_len: 10,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100222c,
        name_start: 19277,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100222d,
        name_start: 19286,
        name_len: 9,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002235,
        name_start: 19295,
        name_len: 7,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002247,
        name_start: 19302,
        name_len: 11,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002248,
        name_start: 19313,
        name_len: 8,
        flags: 0 | HAS_CHAR | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002262,
        name_start: 19321,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002263,
        name_start: 19333,
        name_len: 8,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002800,
        name_start: 19341,
        name_len: 13,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002801,
        name_start: 19354,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002802,
        name_start: 19368,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002803,
        name_start: 19382,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002804,
        name_start: 19397,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002805,
        name_start: 19411,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002806,
        name_start: 19426,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002807,
        name_start: 19441,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002808,
        name_start: 19457,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002809,
        name_start: 19471,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100280a,
        name_start: 19486,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100280b,
        name_start: 19501,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100280c,
        name_start: 19517,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100280d,
        name_start: 19532,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100280e,
        name_start: 19548,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100280f,
        name_start: 19564,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002810,
        name_start: 19581,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002811,
        name_start: 19595,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002812,
        name_start: 19610,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002813,
        name_start: 19625,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002814,
        name_start: 19641,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002815,
        name_start: 19656,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002816,
        name_start: 19672,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002817,
        name_start: 19688,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002818,
        name_start: 19705,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002819,
        name_start: 19720,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100281a,
        name_start: 19736,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100281b,
        name_start: 19752,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100281c,
        name_start: 19769,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100281d,
        name_start: 19785,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100281e,
        name_start: 19802,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100281f,
        name_start: 19819,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002820,
        name_start: 19837,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002821,
        name_start: 19851,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002822,
        name_start: 19866,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002823,
        name_start: 19881,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002824,
        name_start: 19897,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002825,
        name_start: 19912,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002826,
        name_start: 19928,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002827,
        name_start: 19944,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002828,
        name_start: 19961,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002829,
        name_start: 19976,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100282a,
        name_start: 19992,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100282b,
        name_start: 20008,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100282c,
        name_start: 20025,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100282d,
        name_start: 20041,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100282e,
        name_start: 20058,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100282f,
        name_start: 20075,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002830,
        name_start: 20093,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002831,
        name_start: 20108,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002832,
        name_start: 20124,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002833,
        name_start: 20140,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002834,
        name_start: 20157,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002835,
        name_start: 20173,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002836,
        name_start: 20190,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002837,
        name_start: 20207,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002838,
        name_start: 20225,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002839,
        name_start: 20241,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100283a,
        name_start: 20258,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100283b,
        name_start: 20275,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100283c,
        name_start: 20293,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100283d,
        name_start: 20310,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100283e,
        name_start: 20328,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100283f,
        name_start: 20346,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002840,
        name_start: 20365,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002841,
        name_start: 20379,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002842,
        name_start: 20394,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002843,
        name_start: 20409,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002844,
        name_start: 20425,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002845,
        name_start: 20440,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002846,
        name_start: 20456,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002847,
        name_start: 20472,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002848,
        name_start: 20489,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002849,
        name_start: 20504,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100284a,
        name_start: 20520,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100284b,
        name_start: 20536,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100284c,
        name_start: 20553,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100284d,
        name_start: 20569,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100284e,
        name_start: 20586,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100284f,
        name_start: 20603,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002850,
        name_start: 20621,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002851,
        name_start: 20636,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002852,
        name_start: 20652,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002853,
        name_start: 20668,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002854,
        name_start: 20685,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002855,
        name_start: 20701,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002856,
        name_start: 20718,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002857,
        name_start: 20735,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002858,
        name_start: 20753,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002859,
        name_start: 20769,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100285a,
        name_start: 20786,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100285b,
        name_start: 20803,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100285c,
        name_start: 20821,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100285d,
        name_start: 20838,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100285e,
        name_start: 20856,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100285f,
        name_start: 20874,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002860,
        name_start: 20893,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002861,
        name_start: 20908,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002862,
        name_start: 20924,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002863,
        name_start: 20940,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002864,
        name_start: 20957,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002865,
        name_start: 20973,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002866,
        name_start: 20990,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002867,
        name_start: 21007,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002868,
        name_start: 21025,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002869,
        name_start: 21041,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100286a,
        name_start: 21058,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100286b,
        name_start: 21075,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100286c,
        name_start: 21093,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100286d,
        name_start: 21110,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100286e,
        name_start: 21128,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100286f,
        name_start: 21146,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002870,
        name_start: 21165,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002871,
        name_start: 21181,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002872,
        name_start: 21198,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002873,
        name_start: 21215,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002874,
        name_start: 21233,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002875,
        name_start: 21250,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002876,
        name_start: 21268,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002877,
        name_start: 21286,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002878,
        name_start: 21305,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002879,
        name_start: 21322,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100287a,
        name_start: 21340,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100287b,
        name_start: 21358,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100287c,
        name_start: 21377,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100287d,
        name_start: 21395,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100287e,
        name_start: 21414,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100287f,
        name_start: 21433,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002880,
        name_start: 21453,
        name_len: 14,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002881,
        name_start: 21467,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002882,
        name_start: 21482,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002883,
        name_start: 21497,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002884,
        name_start: 21513,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002885,
        name_start: 21528,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002886,
        name_start: 21544,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002887,
        name_start: 21560,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002888,
        name_start: 21577,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002889,
        name_start: 21592,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100288a,
        name_start: 21608,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100288b,
        name_start: 21624,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100288c,
        name_start: 21641,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100288d,
        name_start: 21657,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100288e,
        name_start: 21674,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100288f,
        name_start: 21691,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002890,
        name_start: 21709,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002891,
        name_start: 21724,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002892,
        name_start: 21740,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002893,
        name_start: 21756,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002894,
        name_start: 21773,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002895,
        name_start: 21789,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002896,
        name_start: 21806,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002897,
        name_start: 21823,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002898,
        name_start: 21841,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x01002899,
        name_start: 21857,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100289a,
        name_start: 21874,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100289b,
        name_start: 21891,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100289c,
        name_start: 21909,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100289d,
        name_start: 21926,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100289e,
        name_start: 21944,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0100289f,
        name_start: 21962,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028a0,
        name_start: 21981,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028a1,
        name_start: 21996,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028a2,
        name_start: 22012,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028a3,
        name_start: 22028,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028a4,
        name_start: 22045,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028a5,
        name_start: 22061,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028a6,
        name_start: 22078,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028a7,
        name_start: 22095,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028a8,
        name_start: 22113,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028a9,
        name_start: 22129,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028aa,
        name_start: 22146,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ab,
        name_start: 22163,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ac,
        name_start: 22181,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ad,
        name_start: 22198,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ae,
        name_start: 22216,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028af,
        name_start: 22234,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028b0,
        name_start: 22253,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028b1,
        name_start: 22269,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028b2,
        name_start: 22286,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028b3,
        name_start: 22303,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028b4,
        name_start: 22321,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028b5,
        name_start: 22338,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028b6,
        name_start: 22356,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028b7,
        name_start: 22374,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028b8,
        name_start: 22393,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028b9,
        name_start: 22410,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ba,
        name_start: 22428,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028bb,
        name_start: 22446,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028bc,
        name_start: 22465,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028bd,
        name_start: 22483,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028be,
        name_start: 22502,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028bf,
        name_start: 22521,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028c0,
        name_start: 22541,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028c1,
        name_start: 22556,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028c2,
        name_start: 22572,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028c3,
        name_start: 22588,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028c4,
        name_start: 22605,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028c5,
        name_start: 22621,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028c6,
        name_start: 22638,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028c7,
        name_start: 22655,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028c8,
        name_start: 22673,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028c9,
        name_start: 22689,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ca,
        name_start: 22706,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028cb,
        name_start: 22723,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028cc,
        name_start: 22741,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028cd,
        name_start: 22758,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ce,
        name_start: 22776,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028cf,
        name_start: 22794,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028d0,
        name_start: 22813,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028d1,
        name_start: 22829,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028d2,
        name_start: 22846,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028d3,
        name_start: 22863,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028d4,
        name_start: 22881,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028d5,
        name_start: 22898,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028d6,
        name_start: 22916,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028d7,
        name_start: 22934,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028d8,
        name_start: 22953,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028d9,
        name_start: 22970,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028da,
        name_start: 22988,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028db,
        name_start: 23006,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028dc,
        name_start: 23025,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028dd,
        name_start: 23043,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028de,
        name_start: 23062,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028df,
        name_start: 23081,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028e0,
        name_start: 23101,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028e1,
        name_start: 23117,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028e2,
        name_start: 23134,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028e3,
        name_start: 23151,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028e4,
        name_start: 23169,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028e5,
        name_start: 23186,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028e6,
        name_start: 23204,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028e7,
        name_start: 23222,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028e8,
        name_start: 23241,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028e9,
        name_start: 23258,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ea,
        name_start: 23276,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028eb,
        name_start: 23294,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ec,
        name_start: 23313,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ed,
        name_start: 23331,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ee,
        name_start: 23350,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ef,
        name_start: 23369,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028f0,
        name_start: 23389,
        name_len: 17,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028f1,
        name_start: 23406,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028f2,
        name_start: 23424,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028f3,
        name_start: 23442,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028f4,
        name_start: 23461,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028f5,
        name_start: 23479,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028f6,
        name_start: 23498,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028f7,
        name_start: 23517,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028f8,
        name_start: 23537,
        name_len: 18,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028f9,
        name_start: 23555,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028fa,
        name_start: 23574,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028fb,
        name_start: 23593,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028fc,
        name_start: 23613,
        name_len: 19,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028fd,
        name_start: 23632,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028fe,
        name_start: 23652,
        name_len: 20,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x010028ff,
        name_start: 23672,
        name_len: 21,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100000a8,
        name_start: 23693,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000084c,
        name_start: 23705,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100000a9,
        name_start: 23715,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000084e,
        name_start: 23727,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100000aa,
        name_start: 23737,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000850,
        name_start: 23755,
        name_len: 16,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100000ab,
        name_start: 23771,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000852,
        name_start: 23787,
        name_len: 14,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100000ac,
        name_start: 23801,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000854,
        name_start: 23818,
        name_len: 15,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100000af,
        name_start: 23833,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000856,
        name_start: 23839,
        name_len: 4,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100000be,
        name_start: 23843,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000858,
        name_start: 23852,
        name_len: 7,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100000ee,
        name_start: 23859,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000085a,
        name_start: 23871,
        name_len: 4,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000085a,
        name_start: 23875,
        name_len: 2,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100000f6,
        name_start: 23877,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000085d,
        name_start: 23888,
        name_len: 9,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100000fc,
        name_start: 23897,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000085f,
        name_start: 23904,
        name_len: 5,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000fe22,
        name_start: 23909,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000fe27,
        name_start: 23919,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000fe2c,
        name_start: 23932,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000fe5e,
        name_start: 23947,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000fe60,
        name_start: 23965,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000fe7e,
        name_start: 23978,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000feb0,
        name_start: 23984,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff00,
        name_start: 23996,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff48,
        name_start: 24003,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff49,
        name_start: 24014,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff6c,
        name_start: 24025,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000086b,
        name_start: 24032,
        name_len: 5,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff6d,
        name_start: 24037,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000086d,
        name_start: 24045,
        name_len: 6,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff6e,
        name_start: 24051,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000086f,
        name_start: 24057,
        name_len: 4,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff6f,
        name_start: 24061,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000871,
        name_start: 24072,
        name_len: 9,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff70,
        name_start: 24081,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000873,
        name_start: 24093,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff71,
        name_start: 24103,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000875,
        name_start: 24115,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff72,
        name_start: 24125,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000877,
        name_start: 24137,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff73,
        name_start: 24147,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x00000879,
        name_start: 24159,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff74,
        name_start: 24169,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000087b,
        name_start: 24178,
        name_len: 7,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff75,
        name_start: 24185,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x0000087d,
        name_start: 24197,
        name_len: 10,
        flags: 0 | IS_SECONDARY_IDX | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff76,
        name_start: 24207,
        name_len: 10,
        flags: 0 | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1000ff77,
        name_start: 24217,
        name_len: 10,
        flags: 0 | IS_DEPRECATED,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff02,
        name_start: 24227,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff03,
        name_start: 24234,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff04,
        name_start: 24240,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff07,
        name_start: 24248,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff08,
        name_start: 24258,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff0b,
        name_start: 24270,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff1b,
        name_start: 24278,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff31,
        name_start: 24287,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff32,
        name_start: 24297,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff33,
        name_start: 24312,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff40,
        name_start: 24325,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff41,
        name_start: 24336,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff42,
        name_start: 24345,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff43,
        name_start: 24356,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff44,
        name_start: 24368,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff45,
        name_start: 24379,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff51,
        name_start: 24389,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff52,
        name_start: 24396,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff53,
        name_start: 24401,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff54,
        name_start: 24409,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff57,
        name_start: 24416,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff58,
        name_start: 24426,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff59,
        name_start: 24438,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff5a,
        name_start: 24448,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff5b,
        name_start: 24460,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff5c,
        name_start: 24471,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff5d,
        name_start: 24482,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff5e,
        name_start: 24494,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff60,
        name_start: 24506,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff63,
        name_start: 24515,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff65,
        name_start: 24524,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff67,
        name_start: 24531,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff69,
        name_start: 24538,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff6a,
        name_start: 24547,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff71,
        name_start: 24554,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff72,
        name_start: 24566,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff73,
        name_start: 24580,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff74,
        name_start: 24591,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ff78,
        name_start: 24600,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1004ffff,
        name_start: 24610,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff00,
        name_start: 24619,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff01,
        name_start: 24630,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff02,
        name_start: 24642,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff03,
        name_start: 24653,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff04,
        name_start: 24664,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff05,
        name_start: 24679,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff10,
        name_start: 24692,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff11,
        name_start: 24698,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff60,
        name_start: 24704,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff70,
        name_start: 24714,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff71,
        name_start: 24722,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff72,
        name_start: 24730,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff73,
        name_start: 24737,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff74,
        name_start: 24744,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff75,
        name_start: 24752,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff76,
        name_start: 24758,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff77,
        name_start: 24772,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff78,
        name_start: 24791,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff79,
        name_start: 24803,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff7a,
        name_start: 24822,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff7b,
        name_start: 24837,
        name_len: 23,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff7c,
        name_start: 24860,
        name_len: 23,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1005ff7d,
        name_start: 24883,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100810a4,
        name_start: 24902,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100810ae,
        name_start: 24920,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100810d1,
        name_start: 24928,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100810dc,
        name_start: 24946,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100810f4,
        name_start: 24955,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008c4,
        name_start: 24973,
        name_len: 21,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100810f5,
        name_start: 24994,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081160,
        name_start: 25008,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081162,
        name_start: 25014,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081166,
        name_start: 25022,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081168,
        name_start: 25030,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008116a,
        name_start: 25044,
        name_len: 27,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008116e,
        name_start: 25071,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081170,
        name_start: 25090,
        name_len: 21,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081171,
        name_start: 25111,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081175,
        name_start: 25129,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081177,
        name_start: 25149,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081178,
        name_start: 25164,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081179,
        name_start: 25181,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008117a,
        name_start: 25198,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008117b,
        name_start: 25218,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008117c,
        name_start: 25236,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008117d,
        name_start: 25258,
        name_len: 24,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081180,
        name_start: 25282,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081181,
        name_start: 25301,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081182,
        name_start: 25321,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081183,
        name_start: 25341,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081184,
        name_start: 25356,
        name_len: 23,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081185,
        name_start: 25379,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000008dc,
        name_start: 25386,
        name_len: 18,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081186,
        name_start: 25404,
        name_len: 24,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081188,
        name_start: 25428,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081192,
        name_start: 25437,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081193,
        name_start: 25450,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081199,
        name_start: 25465,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008119b,
        name_start: 25482,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008119d,
        name_start: 25491,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811a0,
        name_start: 25510,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811a4,
        name_start: 25524,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811a6,
        name_start: 25537,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811a8,
        name_start: 25547,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811a9,
        name_start: 25565,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811aa,
        name_start: 25581,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811ac,
        name_start: 25593,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811ad,
        name_start: 25606,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811af,
        name_start: 25621,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811b0,
        name_start: 25638,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811b6,
        name_start: 25652,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811b7,
        name_start: 25667,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811b8,
        name_start: 25682,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811b9,
        name_start: 25698,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811ba,
        name_start: 25716,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811bc,
        name_start: 25726,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811bd,
        name_start: 25748,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811be,
        name_start: 25763,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811bf,
        name_start: 25778,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811d0,
        name_start: 25791,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811d1,
        name_start: 25797,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811d2,
        name_start: 25807,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811d3,
        name_start: 25816,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811d4,
        name_start: 25825,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811d5,
        name_start: 25834,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811d6,
        name_start: 25843,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811d7,
        name_start: 25852,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811d8,
        name_start: 25861,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811d9,
        name_start: 25870,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811da,
        name_start: 25879,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811db,
        name_start: 25888,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811dc,
        name_start: 25898,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811dd,
        name_start: 25908,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811de,
        name_start: 25918,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811df,
        name_start: 25926,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811e0,
        name_start: 25934,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811e1,
        name_start: 25942,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811e2,
        name_start: 25950,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811e3,
        name_start: 25958,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811e4,
        name_start: 25966,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100811e5,
        name_start: 25974,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081200,
        name_start: 25990,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081201,
        name_start: 26002,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081202,
        name_start: 26014,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081203,
        name_start: 26026,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081204,
        name_start: 26038,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081205,
        name_start: 26050,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081206,
        name_start: 26062,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081207,
        name_start: 26074,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081208,
        name_start: 26086,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081209,
        name_start: 26098,
        name_len: 12,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008120a,
        name_start: 26110,
        name_len: 15,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008120b,
        name_start: 26125,
        name_len: 16,
        flags: 0 | HAS_CHAR,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008120c,
        name_start: 26141,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008120d,
        name_start: 26153,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008120e,
        name_start: 26165,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008120f,
        name_start: 26177,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081210,
        name_start: 26189,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081211,
        name_start: 26204,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081215,
        name_start: 26217,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081216,
        name_start: 26233,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081217,
        name_start: 26250,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081218,
        name_start: 26262,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081219,
        name_start: 26276,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008121a,
        name_start: 26290,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008121b,
        name_start: 26305,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008121c,
        name_start: 26320,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008121d,
        name_start: 26336,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008121e,
        name_start: 26355,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081230,
        name_start: 26371,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081232,
        name_start: 26384,
        name_len: 21,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081240,
        name_start: 26405,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081241,
        name_start: 26421,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081242,
        name_start: 26436,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081243,
        name_start: 26447,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081244,
        name_start: 26463,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081245,
        name_start: 26476,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081246,
        name_start: 26491,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081247,
        name_start: 26507,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081249,
        name_start: 26520,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008124a,
        name_start: 26535,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008124b,
        name_start: 26546,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008124c,
        name_start: 26568,
        name_len: 23,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008124d,
        name_start: 26591,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008124e,
        name_start: 26613,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008124f,
        name_start: 26630,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081250,
        name_start: 26646,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081251,
        name_start: 26663,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081252,
        name_start: 26680,
        name_len: 29,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081253,
        name_start: 26709,
        name_len: 30,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081254,
        name_start: 26739,
        name_len: 21,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081255,
        name_start: 26760,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081256,
        name_start: 26780,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081260,
        name_start: 26799,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081261,
        name_start: 26821,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081262,
        name_start: 26843,
        name_len: 27,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081263,
        name_start: 26870,
        name_len: 27,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081264,
        name_start: 26897,
        name_len: 24,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081265,
        name_start: 26921,
        name_len: 24,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081266,
        name_start: 26945,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081267,
        name_start: 26956,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081268,
        name_start: 26969,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081269,
        name_start: 26979,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008126a,
        name_start: 26991,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008126b,
        name_start: 27003,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008126c,
        name_start: 27019,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008126d,
        name_start: 27032,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008126e,
        name_start: 27045,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008126f,
        name_start: 27058,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081270,
        name_start: 27068,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081271,
        name_start: 27084,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081272,
        name_start: 27098,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081273,
        name_start: 27113,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081274,
        name_start: 27120,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081275,
        name_start: 27130,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081276,
        name_start: 27145,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081277,
        name_start: 27160,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081278,
        name_start: 27168,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081279,
        name_start: 27188,
        name_len: 23,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008127a,
        name_start: 27211,
        name_len: 23,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008127b,
        name_start: 27234,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008127c,
        name_start: 27249,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008127d,
        name_start: 27268,
        name_len: 25,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008127e,
        name_start: 27293,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008127f,
        name_start: 27309,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081280,
        name_start: 27316,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081281,
        name_start: 27328,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081282,
        name_start: 27344,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081283,
        name_start: 27364,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081284,
        name_start: 27382,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081285,
        name_start: 27398,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081286,
        name_start: 27418,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081287,
        name_start: 27434,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081288,
        name_start: 27449,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081290,
        name_start: 27460,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081291,
        name_start: 27470,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081292,
        name_start: 27480,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081293,
        name_start: 27490,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081294,
        name_start: 27500,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081295,
        name_start: 27510,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081296,
        name_start: 27520,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081297,
        name_start: 27530,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081298,
        name_start: 27540,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x10081299,
        name_start: 27550,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008129a,
        name_start: 27561,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008129b,
        name_start: 27572,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008129c,
        name_start: 27583,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008129d,
        name_start: 27594,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008129e,
        name_start: 27605,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008129f,
        name_start: 27616,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812a0,
        name_start: 27627,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812a1,
        name_start: 27638,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812a2,
        name_start: 27649,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812a3,
        name_start: 27660,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812a4,
        name_start: 27671,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812a5,
        name_start: 27682,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812a6,
        name_start: 27693,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812a7,
        name_start: 27704,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812a8,
        name_start: 27715,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812a9,
        name_start: 27726,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812aa,
        name_start: 27737,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812ab,
        name_start: 27748,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812ac,
        name_start: 27759,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812ad,
        name_start: 27770,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812b0,
        name_start: 27781,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812b1,
        name_start: 27801,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812b2,
        name_start: 27820,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812b3,
        name_start: 27840,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812b4,
        name_start: 27856,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812b5,
        name_start: 27872,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812b8,
        name_start: 27888,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812b9,
        name_start: 27903,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812ba,
        name_start: 27918,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812bb,
        name_start: 27933,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812bc,
        name_start: 27948,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x100812bd,
        name_start: 27963,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe01,
        name_start: 27982,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe02,
        name_start: 27997,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe03,
        name_start: 28012,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe04,
        name_start: 28027,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe05,
        name_start: 28042,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe06,
        name_start: 28057,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe07,
        name_start: 28072,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe08,
        name_start: 28087,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe09,
        name_start: 28102,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe0a,
        name_start: 28117,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe0b,
        name_start: 28133,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe0c,
        name_start: 28149,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe20,
        name_start: 28165,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe21,
        name_start: 28175,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe22,
        name_start: 28188,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe23,
        name_start: 28202,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe24,
        name_start: 28216,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008fe25,
        name_start: 28233,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff01,
        name_start: 28248,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff02,
        name_start: 28260,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff03,
        name_start: 28279,
        name_len: 21,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff04,
        name_start: 28300,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff05,
        name_start: 28317,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff06,
        name_start: 28336,
        name_len: 21,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff07,
        name_start: 28357,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff10,
        name_start: 28379,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff11,
        name_start: 28390,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff12,
        name_start: 28410,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff13,
        name_start: 28423,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff14,
        name_start: 28443,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff15,
        name_start: 28456,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff16,
        name_start: 28469,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff17,
        name_start: 28482,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff18,
        name_start: 28495,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff19,
        name_start: 28507,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff1a,
        name_start: 28515,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff1b,
        name_start: 28524,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff1c,
        name_start: 28534,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff1d,
        name_start: 28549,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff1e,
        name_start: 28563,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff1f,
        name_start: 28571,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff20,
        name_start: 28583,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff21,
        name_start: 28595,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff22,
        name_start: 28608,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff23,
        name_start: 28626,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff24,
        name_start: 28638,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff25,
        name_start: 28652,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff26,
        name_start: 28667,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff27,
        name_start: 28675,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff28,
        name_start: 28686,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff29,
        name_start: 28694,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff2a,
        name_start: 28705,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff2b,
        name_start: 28717,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff2c,
        name_start: 28727,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff2d,
        name_start: 28736,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff2e,
        name_start: 28751,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff2f,
        name_start: 28758,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff30,
        name_start: 28767,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff31,
        name_start: 28780,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff32,
        name_start: 28794,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff33,
        name_start: 28808,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff34,
        name_start: 28822,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff35,
        name_start: 28836,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff36,
        name_start: 28849,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff37,
        name_start: 28857,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff38,
        name_start: 28868,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff39,
        name_start: 28879,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff3a,
        name_start: 28894,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff3b,
        name_start: 28906,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff3c,
        name_start: 28926,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff3d,
        name_start: 28937,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff3e,
        name_start: 28950,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff3f,
        name_start: 28965,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff40,
        name_start: 28980,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff41,
        name_start: 28991,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff42,
        name_start: 29002,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff43,
        name_start: 29013,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff44,
        name_start: 29024,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff45,
        name_start: 29035,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff46,
        name_start: 29046,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff47,
        name_start: 29057,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff48,
        name_start: 29068,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff49,
        name_start: 29079,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff4a,
        name_start: 29090,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff4b,
        name_start: 29101,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff4c,
        name_start: 29112,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff4d,
        name_start: 29123,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff4e,
        name_start: 29134,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff4f,
        name_start: 29145,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff50,
        name_start: 29156,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff51,
        name_start: 29175,
        name_len: 20,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff52,
        name_start: 29195,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff53,
        name_start: 29203,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x000009f1,
        name_start: 29209,
        name_len: 17,
        flags: 0 | IS_SECONDARY_IDX,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff54,
        name_start: 29226,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff55,
        name_start: 29240,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff56,
        name_start: 29249,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff57,
        name_start: 29258,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff58,
        name_start: 29266,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff59,
        name_start: 29273,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff5a,
        name_start: 29284,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff5b,
        name_start: 29291,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff5c,
        name_start: 29304,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff5d,
        name_start: 29313,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff5e,
        name_start: 29325,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff5f,
        name_start: 29333,
        name_len: 6,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff60,
        name_start: 29339,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff61,
        name_start: 29349,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff62,
        name_start: 29359,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff63,
        name_start: 29369,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff65,
        name_start: 29380,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff66,
        name_start: 29390,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff67,
        name_start: 29400,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff68,
        name_start: 29411,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff69,
        name_start: 29418,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff6a,
        name_start: 29426,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff6b,
        name_start: 29440,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff6c,
        name_start: 29448,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff6d,
        name_start: 29458,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff6e,
        name_start: 29467,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff70,
        name_start: 29476,
        name_len: 5,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff72,
        name_start: 29481,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff73,
        name_start: 29490,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff74,
        name_start: 29500,
        name_len: 17,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff75,
        name_start: 29517,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff76,
        name_start: 29531,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff77,
        name_start: 29545,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff78,
        name_start: 29553,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff79,
        name_start: 29565,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff7a,
        name_start: 29579,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff7b,
        name_start: 29594,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff7c,
        name_start: 29602,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff7d,
        name_start: 29611,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff7e,
        name_start: 29626,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff7f,
        name_start: 29637,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff80,
        name_start: 29649,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff81,
        name_start: 29661,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff82,
        name_start: 29670,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff84,
        name_start: 29680,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff85,
        name_start: 29690,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff86,
        name_start: 29701,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff87,
        name_start: 29712,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff88,
        name_start: 29721,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff89,
        name_start: 29736,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff8a,
        name_start: 29744,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff8b,
        name_start: 29752,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff8c,
        name_start: 29762,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff8d,
        name_start: 29773,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff8e,
        name_start: 29781,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff8f,
        name_start: 29794,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff90,
        name_start: 29804,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff91,
        name_start: 29819,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff92,
        name_start: 29831,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff93,
        name_start: 29840,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff94,
        name_start: 29851,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff95,
        name_start: 29864,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff96,
        name_start: 29872,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff97,
        name_start: 29879,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff98,
        name_start: 29895,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff99,
        name_start: 29910,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff9a,
        name_start: 29929,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff9b,
        name_start: 29941,
        name_len: 19,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff9c,
        name_start: 29960,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff9d,
        name_start: 29974,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff9e,
        name_start: 29987,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ff9f,
        name_start: 30003,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffa0,
        name_start: 30011,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffa1,
        name_start: 30021,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffa2,
        name_start: 30029,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffa3,
        name_start: 30040,
        name_len: 7,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffa4,
        name_start: 30047,
        name_len: 9,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffa5,
        name_start: 30056,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffa6,
        name_start: 30066,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffa7,
        name_start: 30074,
        name_len: 11,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffa8,
        name_start: 30085,
        name_len: 13,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffa9,
        name_start: 30098,
        name_len: 18,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffb0,
        name_start: 30116,
        name_len: 14,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffb1,
        name_start: 30130,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffb2,
        name_start: 30145,
        name_len: 16,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffb3,
        name_start: 30161,
        name_len: 12,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffb4,
        name_start: 30173,
        name_len: 8,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffb5,
        name_start: 30181,
        name_len: 10,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffb6,
        name_start: 30191,
        name_len: 15,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffb7,
        name_start: 30206,
        name_len: 22,
        flags: 0,
    },
    KeysymData {
        keysym_or_definitive_idx: 0x1008ffb8,
        name_start: 30228,
        name_len: 14,
        flags: 0,
    },
];

pub mod syms {
    #![expect(non_upper_case_globals, reason = "a keysym's constant is its X11 name, which is how every keymap spells it")]
    use super::*;
    pub const NoSymbol: Keysym = Keysym(0x00000000);
    pub const space: Keysym = Keysym(0x00000020);
    pub const exclam: Keysym = Keysym(0x00000021);
    pub const quotedbl: Keysym = Keysym(0x00000022);
    pub const numbersign: Keysym = Keysym(0x00000023);
    pub const dollar: Keysym = Keysym(0x00000024);
    pub const percent: Keysym = Keysym(0x00000025);
    pub const ampersand: Keysym = Keysym(0x00000026);
    pub const apostrophe: Keysym = Keysym(0x00000027);
    pub const quoteright: Keysym = Keysym(0x00000027);
    pub const parenleft: Keysym = Keysym(0x00000028);
    pub const parenright: Keysym = Keysym(0x00000029);
    pub const asterisk: Keysym = Keysym(0x0000002a);
    pub const plus: Keysym = Keysym(0x0000002b);
    pub const comma: Keysym = Keysym(0x0000002c);
    pub const minus: Keysym = Keysym(0x0000002d);
    pub const period: Keysym = Keysym(0x0000002e);
    pub const slash: Keysym = Keysym(0x0000002f);
    pub const _0: Keysym = Keysym(0x00000030);
    pub const _1: Keysym = Keysym(0x00000031);
    pub const _2: Keysym = Keysym(0x00000032);
    pub const _3: Keysym = Keysym(0x00000033);
    pub const _4: Keysym = Keysym(0x00000034);
    pub const _5: Keysym = Keysym(0x00000035);
    pub const _6: Keysym = Keysym(0x00000036);
    pub const _7: Keysym = Keysym(0x00000037);
    pub const _8: Keysym = Keysym(0x00000038);
    pub const _9: Keysym = Keysym(0x00000039);
    pub const colon: Keysym = Keysym(0x0000003a);
    pub const semicolon: Keysym = Keysym(0x0000003b);
    pub const less: Keysym = Keysym(0x0000003c);
    pub const equal: Keysym = Keysym(0x0000003d);
    pub const greater: Keysym = Keysym(0x0000003e);
    pub const question: Keysym = Keysym(0x0000003f);
    pub const at: Keysym = Keysym(0x00000040);
    pub const A: Keysym = Keysym(0x00000041);
    pub const B: Keysym = Keysym(0x00000042);
    pub const C: Keysym = Keysym(0x00000043);
    pub const D: Keysym = Keysym(0x00000044);
    pub const E: Keysym = Keysym(0x00000045);
    pub const F: Keysym = Keysym(0x00000046);
    pub const G: Keysym = Keysym(0x00000047);
    pub const H: Keysym = Keysym(0x00000048);
    pub const I: Keysym = Keysym(0x00000049);
    pub const J: Keysym = Keysym(0x0000004a);
    pub const K: Keysym = Keysym(0x0000004b);
    pub const L: Keysym = Keysym(0x0000004c);
    pub const M: Keysym = Keysym(0x0000004d);
    pub const N: Keysym = Keysym(0x0000004e);
    pub const O: Keysym = Keysym(0x0000004f);
    pub const P: Keysym = Keysym(0x00000050);
    pub const Q: Keysym = Keysym(0x00000051);
    pub const R: Keysym = Keysym(0x00000052);
    pub const S: Keysym = Keysym(0x00000053);
    pub const T: Keysym = Keysym(0x00000054);
    pub const U: Keysym = Keysym(0x00000055);
    pub const V: Keysym = Keysym(0x00000056);
    pub const W: Keysym = Keysym(0x00000057);
    pub const X: Keysym = Keysym(0x00000058);
    pub const Y: Keysym = Keysym(0x00000059);
    pub const Z: Keysym = Keysym(0x0000005a);
    pub const bracketleft: Keysym = Keysym(0x0000005b);
    pub const backslash: Keysym = Keysym(0x0000005c);
    pub const bracketright: Keysym = Keysym(0x0000005d);
    pub const asciicircum: Keysym = Keysym(0x0000005e);
    pub const underscore: Keysym = Keysym(0x0000005f);
    pub const grave: Keysym = Keysym(0x00000060);
    pub const quoteleft: Keysym = Keysym(0x00000060);
    pub const a: Keysym = Keysym(0x00000061);
    pub const b: Keysym = Keysym(0x00000062);
    pub const c: Keysym = Keysym(0x00000063);
    pub const d: Keysym = Keysym(0x00000064);
    pub const e: Keysym = Keysym(0x00000065);
    pub const f: Keysym = Keysym(0x00000066);
    pub const g: Keysym = Keysym(0x00000067);
    pub const h: Keysym = Keysym(0x00000068);
    pub const i: Keysym = Keysym(0x00000069);
    pub const j: Keysym = Keysym(0x0000006a);
    pub const k: Keysym = Keysym(0x0000006b);
    pub const l: Keysym = Keysym(0x0000006c);
    pub const m: Keysym = Keysym(0x0000006d);
    pub const n: Keysym = Keysym(0x0000006e);
    pub const o: Keysym = Keysym(0x0000006f);
    pub const p: Keysym = Keysym(0x00000070);
    pub const q: Keysym = Keysym(0x00000071);
    pub const r: Keysym = Keysym(0x00000072);
    pub const s: Keysym = Keysym(0x00000073);
    pub const t: Keysym = Keysym(0x00000074);
    pub const u: Keysym = Keysym(0x00000075);
    pub const v: Keysym = Keysym(0x00000076);
    pub const w: Keysym = Keysym(0x00000077);
    pub const x: Keysym = Keysym(0x00000078);
    pub const y: Keysym = Keysym(0x00000079);
    pub const z: Keysym = Keysym(0x0000007a);
    pub const braceleft: Keysym = Keysym(0x0000007b);
    pub const bar: Keysym = Keysym(0x0000007c);
    pub const braceright: Keysym = Keysym(0x0000007d);
    pub const asciitilde: Keysym = Keysym(0x0000007e);
    pub const nobreakspace: Keysym = Keysym(0x000000a0);
    pub const exclamdown: Keysym = Keysym(0x000000a1);
    pub const cent: Keysym = Keysym(0x000000a2);
    pub const sterling: Keysym = Keysym(0x000000a3);
    pub const currency: Keysym = Keysym(0x000000a4);
    pub const yen: Keysym = Keysym(0x000000a5);
    pub const brokenbar: Keysym = Keysym(0x000000a6);
    pub const section: Keysym = Keysym(0x000000a7);
    pub const diaeresis: Keysym = Keysym(0x000000a8);
    pub const copyright: Keysym = Keysym(0x000000a9);
    pub const ordfeminine: Keysym = Keysym(0x000000aa);
    pub const guillemotleft: Keysym = Keysym(0x000000ab);
    pub const guillemetleft: Keysym = Keysym(0x000000ab);
    pub const notsign: Keysym = Keysym(0x000000ac);
    pub const hyphen: Keysym = Keysym(0x000000ad);
    pub const registered: Keysym = Keysym(0x000000ae);
    pub const macron: Keysym = Keysym(0x000000af);
    pub const degree: Keysym = Keysym(0x000000b0);
    pub const plusminus: Keysym = Keysym(0x000000b1);
    pub const twosuperior: Keysym = Keysym(0x000000b2);
    pub const threesuperior: Keysym = Keysym(0x000000b3);
    pub const acute: Keysym = Keysym(0x000000b4);
    pub const mu: Keysym = Keysym(0x000000b5);
    pub const paragraph: Keysym = Keysym(0x000000b6);
    pub const periodcentered: Keysym = Keysym(0x000000b7);
    pub const cedilla: Keysym = Keysym(0x000000b8);
    pub const onesuperior: Keysym = Keysym(0x000000b9);
    pub const masculine: Keysym = Keysym(0x000000ba);
    pub const ordmasculine: Keysym = Keysym(0x000000ba);
    pub const guillemotright: Keysym = Keysym(0x000000bb);
    pub const guillemetright: Keysym = Keysym(0x000000bb);
    pub const onequarter: Keysym = Keysym(0x000000bc);
    pub const onehalf: Keysym = Keysym(0x000000bd);
    pub const threequarters: Keysym = Keysym(0x000000be);
    pub const questiondown: Keysym = Keysym(0x000000bf);
    pub const Agrave: Keysym = Keysym(0x000000c0);
    pub const Aacute: Keysym = Keysym(0x000000c1);
    pub const Acircumflex: Keysym = Keysym(0x000000c2);
    pub const Atilde: Keysym = Keysym(0x000000c3);
    pub const Adiaeresis: Keysym = Keysym(0x000000c4);
    pub const Aring: Keysym = Keysym(0x000000c5);
    pub const AE: Keysym = Keysym(0x000000c6);
    pub const Ccedilla: Keysym = Keysym(0x000000c7);
    pub const Egrave: Keysym = Keysym(0x000000c8);
    pub const Eacute: Keysym = Keysym(0x000000c9);
    pub const Ecircumflex: Keysym = Keysym(0x000000ca);
    pub const Ediaeresis: Keysym = Keysym(0x000000cb);
    pub const Igrave: Keysym = Keysym(0x000000cc);
    pub const Iacute: Keysym = Keysym(0x000000cd);
    pub const Icircumflex: Keysym = Keysym(0x000000ce);
    pub const Idiaeresis: Keysym = Keysym(0x000000cf);
    pub const ETH: Keysym = Keysym(0x000000d0);
    pub const Eth: Keysym = Keysym(0x000000d0);
    pub const Ntilde: Keysym = Keysym(0x000000d1);
    pub const Ograve: Keysym = Keysym(0x000000d2);
    pub const Oacute: Keysym = Keysym(0x000000d3);
    pub const Ocircumflex: Keysym = Keysym(0x000000d4);
    pub const Otilde: Keysym = Keysym(0x000000d5);
    pub const Odiaeresis: Keysym = Keysym(0x000000d6);
    pub const multiply: Keysym = Keysym(0x000000d7);
    pub const Oslash: Keysym = Keysym(0x000000d8);
    pub const Ooblique: Keysym = Keysym(0x000000d8);
    pub const Ugrave: Keysym = Keysym(0x000000d9);
    pub const Uacute: Keysym = Keysym(0x000000da);
    pub const Ucircumflex: Keysym = Keysym(0x000000db);
    pub const Udiaeresis: Keysym = Keysym(0x000000dc);
    pub const Yacute: Keysym = Keysym(0x000000dd);
    pub const THORN: Keysym = Keysym(0x000000de);
    pub const Thorn: Keysym = Keysym(0x000000de);
    pub const ssharp: Keysym = Keysym(0x000000df);
    pub const agrave: Keysym = Keysym(0x000000e0);
    pub const aacute: Keysym = Keysym(0x000000e1);
    pub const acircumflex: Keysym = Keysym(0x000000e2);
    pub const atilde: Keysym = Keysym(0x000000e3);
    pub const adiaeresis: Keysym = Keysym(0x000000e4);
    pub const aring: Keysym = Keysym(0x000000e5);
    pub const ae: Keysym = Keysym(0x000000e6);
    pub const ccedilla: Keysym = Keysym(0x000000e7);
    pub const egrave: Keysym = Keysym(0x000000e8);
    pub const eacute: Keysym = Keysym(0x000000e9);
    pub const ecircumflex: Keysym = Keysym(0x000000ea);
    pub const ediaeresis: Keysym = Keysym(0x000000eb);
    pub const igrave: Keysym = Keysym(0x000000ec);
    pub const iacute: Keysym = Keysym(0x000000ed);
    pub const icircumflex: Keysym = Keysym(0x000000ee);
    pub const idiaeresis: Keysym = Keysym(0x000000ef);
    pub const eth: Keysym = Keysym(0x000000f0);
    pub const ntilde: Keysym = Keysym(0x000000f1);
    pub const ograve: Keysym = Keysym(0x000000f2);
    pub const oacute: Keysym = Keysym(0x000000f3);
    pub const ocircumflex: Keysym = Keysym(0x000000f4);
    pub const otilde: Keysym = Keysym(0x000000f5);
    pub const odiaeresis: Keysym = Keysym(0x000000f6);
    pub const division: Keysym = Keysym(0x000000f7);
    pub const oslash: Keysym = Keysym(0x000000f8);
    pub const ooblique: Keysym = Keysym(0x000000f8);
    pub const ugrave: Keysym = Keysym(0x000000f9);
    pub const uacute: Keysym = Keysym(0x000000fa);
    pub const ucircumflex: Keysym = Keysym(0x000000fb);
    pub const udiaeresis: Keysym = Keysym(0x000000fc);
    pub const yacute: Keysym = Keysym(0x000000fd);
    pub const thorn: Keysym = Keysym(0x000000fe);
    pub const ydiaeresis: Keysym = Keysym(0x000000ff);
    pub const Aogonek: Keysym = Keysym(0x000001a1);
    pub const breve: Keysym = Keysym(0x000001a2);
    pub const Lstroke: Keysym = Keysym(0x000001a3);
    pub const Lcaron: Keysym = Keysym(0x000001a5);
    pub const Sacute: Keysym = Keysym(0x000001a6);
    pub const Scaron: Keysym = Keysym(0x000001a9);
    pub const Scedilla: Keysym = Keysym(0x000001aa);
    pub const Tcaron: Keysym = Keysym(0x000001ab);
    pub const Zacute: Keysym = Keysym(0x000001ac);
    pub const Zcaron: Keysym = Keysym(0x000001ae);
    pub const Zabovedot: Keysym = Keysym(0x000001af);
    pub const aogonek: Keysym = Keysym(0x000001b1);
    pub const ogonek: Keysym = Keysym(0x000001b2);
    pub const lstroke: Keysym = Keysym(0x000001b3);
    pub const lcaron: Keysym = Keysym(0x000001b5);
    pub const sacute: Keysym = Keysym(0x000001b6);
    pub const caron: Keysym = Keysym(0x000001b7);
    pub const scaron: Keysym = Keysym(0x000001b9);
    pub const scedilla: Keysym = Keysym(0x000001ba);
    pub const tcaron: Keysym = Keysym(0x000001bb);
    pub const zacute: Keysym = Keysym(0x000001bc);
    pub const doubleacute: Keysym = Keysym(0x000001bd);
    pub const zcaron: Keysym = Keysym(0x000001be);
    pub const zabovedot: Keysym = Keysym(0x000001bf);
    pub const Racute: Keysym = Keysym(0x000001c0);
    pub const Abreve: Keysym = Keysym(0x000001c3);
    pub const Lacute: Keysym = Keysym(0x000001c5);
    pub const Cacute: Keysym = Keysym(0x000001c6);
    pub const Ccaron: Keysym = Keysym(0x000001c8);
    pub const Eogonek: Keysym = Keysym(0x000001ca);
    pub const Ecaron: Keysym = Keysym(0x000001cc);
    pub const Dcaron: Keysym = Keysym(0x000001cf);
    pub const Dstroke: Keysym = Keysym(0x000001d0);
    pub const Nacute: Keysym = Keysym(0x000001d1);
    pub const Ncaron: Keysym = Keysym(0x000001d2);
    pub const Odoubleacute: Keysym = Keysym(0x000001d5);
    pub const Rcaron: Keysym = Keysym(0x000001d8);
    pub const Uring: Keysym = Keysym(0x000001d9);
    pub const Udoubleacute: Keysym = Keysym(0x000001db);
    pub const Tcedilla: Keysym = Keysym(0x000001de);
    pub const racute: Keysym = Keysym(0x000001e0);
    pub const abreve: Keysym = Keysym(0x000001e3);
    pub const lacute: Keysym = Keysym(0x000001e5);
    pub const cacute: Keysym = Keysym(0x000001e6);
    pub const ccaron: Keysym = Keysym(0x000001e8);
    pub const eogonek: Keysym = Keysym(0x000001ea);
    pub const ecaron: Keysym = Keysym(0x000001ec);
    pub const dcaron: Keysym = Keysym(0x000001ef);
    pub const dstroke: Keysym = Keysym(0x000001f0);
    pub const nacute: Keysym = Keysym(0x000001f1);
    pub const ncaron: Keysym = Keysym(0x000001f2);
    pub const odoubleacute: Keysym = Keysym(0x000001f5);
    pub const rcaron: Keysym = Keysym(0x000001f8);
    pub const uring: Keysym = Keysym(0x000001f9);
    pub const udoubleacute: Keysym = Keysym(0x000001fb);
    pub const tcedilla: Keysym = Keysym(0x000001fe);
    pub const abovedot: Keysym = Keysym(0x000001ff);
    pub const Hstroke: Keysym = Keysym(0x000002a1);
    pub const Hcircumflex: Keysym = Keysym(0x000002a6);
    pub const Iabovedot: Keysym = Keysym(0x000002a9);
    pub const Gbreve: Keysym = Keysym(0x000002ab);
    pub const Jcircumflex: Keysym = Keysym(0x000002ac);
    pub const hstroke: Keysym = Keysym(0x000002b1);
    pub const hcircumflex: Keysym = Keysym(0x000002b6);
    pub const idotless: Keysym = Keysym(0x000002b9);
    pub const gbreve: Keysym = Keysym(0x000002bb);
    pub const jcircumflex: Keysym = Keysym(0x000002bc);
    pub const Cabovedot: Keysym = Keysym(0x000002c5);
    pub const Ccircumflex: Keysym = Keysym(0x000002c6);
    pub const Gabovedot: Keysym = Keysym(0x000002d5);
    pub const Gcircumflex: Keysym = Keysym(0x000002d8);
    pub const Ubreve: Keysym = Keysym(0x000002dd);
    pub const Scircumflex: Keysym = Keysym(0x000002de);
    pub const cabovedot: Keysym = Keysym(0x000002e5);
    pub const ccircumflex: Keysym = Keysym(0x000002e6);
    pub const gabovedot: Keysym = Keysym(0x000002f5);
    pub const gcircumflex: Keysym = Keysym(0x000002f8);
    pub const ubreve: Keysym = Keysym(0x000002fd);
    pub const scircumflex: Keysym = Keysym(0x000002fe);
    pub const kra: Keysym = Keysym(0x000003a2);
    pub const kappa: Keysym = Keysym(0x000003a2);
    pub const Rcedilla: Keysym = Keysym(0x000003a3);
    pub const Itilde: Keysym = Keysym(0x000003a5);
    pub const Lcedilla: Keysym = Keysym(0x000003a6);
    pub const Emacron: Keysym = Keysym(0x000003aa);
    pub const Gcedilla: Keysym = Keysym(0x000003ab);
    pub const Tslash: Keysym = Keysym(0x000003ac);
    pub const rcedilla: Keysym = Keysym(0x000003b3);
    pub const itilde: Keysym = Keysym(0x000003b5);
    pub const lcedilla: Keysym = Keysym(0x000003b6);
    pub const emacron: Keysym = Keysym(0x000003ba);
    pub const gcedilla: Keysym = Keysym(0x000003bb);
    pub const tslash: Keysym = Keysym(0x000003bc);
    pub const ENG: Keysym = Keysym(0x000003bd);
    pub const eng: Keysym = Keysym(0x000003bf);
    pub const Amacron: Keysym = Keysym(0x000003c0);
    pub const Iogonek: Keysym = Keysym(0x000003c7);
    pub const Eabovedot: Keysym = Keysym(0x000003cc);
    pub const Imacron: Keysym = Keysym(0x000003cf);
    pub const Ncedilla: Keysym = Keysym(0x000003d1);
    pub const Omacron: Keysym = Keysym(0x000003d2);
    pub const Kcedilla: Keysym = Keysym(0x000003d3);
    pub const Uogonek: Keysym = Keysym(0x000003d9);
    pub const Utilde: Keysym = Keysym(0x000003dd);
    pub const Umacron: Keysym = Keysym(0x000003de);
    pub const amacron: Keysym = Keysym(0x000003e0);
    pub const iogonek: Keysym = Keysym(0x000003e7);
    pub const eabovedot: Keysym = Keysym(0x000003ec);
    pub const imacron: Keysym = Keysym(0x000003ef);
    pub const ncedilla: Keysym = Keysym(0x000003f1);
    pub const omacron: Keysym = Keysym(0x000003f2);
    pub const kcedilla: Keysym = Keysym(0x000003f3);
    pub const uogonek: Keysym = Keysym(0x000003f9);
    pub const utilde: Keysym = Keysym(0x000003fd);
    pub const umacron: Keysym = Keysym(0x000003fe);
    pub const overline: Keysym = Keysym(0x0000047e);
    pub const kana_fullstop: Keysym = Keysym(0x000004a1);
    pub const kana_openingbracket: Keysym = Keysym(0x000004a2);
    pub const kana_closingbracket: Keysym = Keysym(0x000004a3);
    pub const kana_comma: Keysym = Keysym(0x000004a4);
    pub const kana_conjunctive: Keysym = Keysym(0x000004a5);
    pub const kana_middledot: Keysym = Keysym(0x000004a5);
    pub const kana_WO: Keysym = Keysym(0x000004a6);
    pub const kana_a: Keysym = Keysym(0x000004a7);
    pub const kana_i: Keysym = Keysym(0x000004a8);
    pub const kana_u: Keysym = Keysym(0x000004a9);
    pub const kana_e: Keysym = Keysym(0x000004aa);
    pub const kana_o: Keysym = Keysym(0x000004ab);
    pub const kana_ya: Keysym = Keysym(0x000004ac);
    pub const kana_yu: Keysym = Keysym(0x000004ad);
    pub const kana_yo: Keysym = Keysym(0x000004ae);
    pub const kana_tsu: Keysym = Keysym(0x000004af);
    pub const kana_tu: Keysym = Keysym(0x000004af);
    pub const prolongedsound: Keysym = Keysym(0x000004b0);
    pub const kana_A: Keysym = Keysym(0x000004b1);
    pub const kana_I: Keysym = Keysym(0x000004b2);
    pub const kana_U: Keysym = Keysym(0x000004b3);
    pub const kana_E: Keysym = Keysym(0x000004b4);
    pub const kana_O: Keysym = Keysym(0x000004b5);
    pub const kana_KA: Keysym = Keysym(0x000004b6);
    pub const kana_KI: Keysym = Keysym(0x000004b7);
    pub const kana_KU: Keysym = Keysym(0x000004b8);
    pub const kana_KE: Keysym = Keysym(0x000004b9);
    pub const kana_KO: Keysym = Keysym(0x000004ba);
    pub const kana_SA: Keysym = Keysym(0x000004bb);
    pub const kana_SHI: Keysym = Keysym(0x000004bc);
    pub const kana_SU: Keysym = Keysym(0x000004bd);
    pub const kana_SE: Keysym = Keysym(0x000004be);
    pub const kana_SO: Keysym = Keysym(0x000004bf);
    pub const kana_TA: Keysym = Keysym(0x000004c0);
    pub const kana_CHI: Keysym = Keysym(0x000004c1);
    pub const kana_TI: Keysym = Keysym(0x000004c1);
    pub const kana_TSU: Keysym = Keysym(0x000004c2);
    pub const kana_TU: Keysym = Keysym(0x000004c2);
    pub const kana_TE: Keysym = Keysym(0x000004c3);
    pub const kana_TO: Keysym = Keysym(0x000004c4);
    pub const kana_NA: Keysym = Keysym(0x000004c5);
    pub const kana_NI: Keysym = Keysym(0x000004c6);
    pub const kana_NU: Keysym = Keysym(0x000004c7);
    pub const kana_NE: Keysym = Keysym(0x000004c8);
    pub const kana_NO: Keysym = Keysym(0x000004c9);
    pub const kana_HA: Keysym = Keysym(0x000004ca);
    pub const kana_HI: Keysym = Keysym(0x000004cb);
    pub const kana_FU: Keysym = Keysym(0x000004cc);
    pub const kana_HU: Keysym = Keysym(0x000004cc);
    pub const kana_HE: Keysym = Keysym(0x000004cd);
    pub const kana_HO: Keysym = Keysym(0x000004ce);
    pub const kana_MA: Keysym = Keysym(0x000004cf);
    pub const kana_MI: Keysym = Keysym(0x000004d0);
    pub const kana_MU: Keysym = Keysym(0x000004d1);
    pub const kana_ME: Keysym = Keysym(0x000004d2);
    pub const kana_MO: Keysym = Keysym(0x000004d3);
    pub const kana_YA: Keysym = Keysym(0x000004d4);
    pub const kana_YU: Keysym = Keysym(0x000004d5);
    pub const kana_YO: Keysym = Keysym(0x000004d6);
    pub const kana_RA: Keysym = Keysym(0x000004d7);
    pub const kana_RI: Keysym = Keysym(0x000004d8);
    pub const kana_RU: Keysym = Keysym(0x000004d9);
    pub const kana_RE: Keysym = Keysym(0x000004da);
    pub const kana_RO: Keysym = Keysym(0x000004db);
    pub const kana_WA: Keysym = Keysym(0x000004dc);
    pub const kana_N: Keysym = Keysym(0x000004dd);
    pub const voicedsound: Keysym = Keysym(0x000004de);
    pub const semivoicedsound: Keysym = Keysym(0x000004df);
    pub const Arabic_comma: Keysym = Keysym(0x000005ac);
    pub const Arabic_semicolon: Keysym = Keysym(0x000005bb);
    pub const Arabic_question_mark: Keysym = Keysym(0x000005bf);
    pub const Arabic_hamza: Keysym = Keysym(0x000005c1);
    pub const Arabic_maddaonalef: Keysym = Keysym(0x000005c2);
    pub const Arabic_hamzaonalef: Keysym = Keysym(0x000005c3);
    pub const Arabic_hamzaonwaw: Keysym = Keysym(0x000005c4);
    pub const Arabic_hamzaunderalef: Keysym = Keysym(0x000005c5);
    pub const Arabic_hamzaonyeh: Keysym = Keysym(0x000005c6);
    pub const Arabic_alef: Keysym = Keysym(0x000005c7);
    pub const Arabic_beh: Keysym = Keysym(0x000005c8);
    pub const Arabic_tehmarbuta: Keysym = Keysym(0x000005c9);
    pub const Arabic_teh: Keysym = Keysym(0x000005ca);
    pub const Arabic_theh: Keysym = Keysym(0x000005cb);
    pub const Arabic_jeem: Keysym = Keysym(0x000005cc);
    pub const Arabic_hah: Keysym = Keysym(0x000005cd);
    pub const Arabic_khah: Keysym = Keysym(0x000005ce);
    pub const Arabic_dal: Keysym = Keysym(0x000005cf);
    pub const Arabic_thal: Keysym = Keysym(0x000005d0);
    pub const Arabic_ra: Keysym = Keysym(0x000005d1);
    pub const Arabic_zain: Keysym = Keysym(0x000005d2);
    pub const Arabic_seen: Keysym = Keysym(0x000005d3);
    pub const Arabic_sheen: Keysym = Keysym(0x000005d4);
    pub const Arabic_sad: Keysym = Keysym(0x000005d5);
    pub const Arabic_dad: Keysym = Keysym(0x000005d6);
    pub const Arabic_tah: Keysym = Keysym(0x000005d7);
    pub const Arabic_zah: Keysym = Keysym(0x000005d8);
    pub const Arabic_ain: Keysym = Keysym(0x000005d9);
    pub const Arabic_ghain: Keysym = Keysym(0x000005da);
    pub const Arabic_tatweel: Keysym = Keysym(0x000005e0);
    pub const Arabic_feh: Keysym = Keysym(0x000005e1);
    pub const Arabic_qaf: Keysym = Keysym(0x000005e2);
    pub const Arabic_kaf: Keysym = Keysym(0x000005e3);
    pub const Arabic_lam: Keysym = Keysym(0x000005e4);
    pub const Arabic_meem: Keysym = Keysym(0x000005e5);
    pub const Arabic_noon: Keysym = Keysym(0x000005e6);
    pub const Arabic_ha: Keysym = Keysym(0x000005e7);
    pub const Arabic_heh: Keysym = Keysym(0x000005e7);
    pub const Arabic_waw: Keysym = Keysym(0x000005e8);
    pub const Arabic_alefmaksura: Keysym = Keysym(0x000005e9);
    pub const Arabic_yeh: Keysym = Keysym(0x000005ea);
    pub const Arabic_fathatan: Keysym = Keysym(0x000005eb);
    pub const Arabic_dammatan: Keysym = Keysym(0x000005ec);
    pub const Arabic_kasratan: Keysym = Keysym(0x000005ed);
    pub const Arabic_fatha: Keysym = Keysym(0x000005ee);
    pub const Arabic_damma: Keysym = Keysym(0x000005ef);
    pub const Arabic_kasra: Keysym = Keysym(0x000005f0);
    pub const Arabic_shadda: Keysym = Keysym(0x000005f1);
    pub const Arabic_sukun: Keysym = Keysym(0x000005f2);
    pub const Serbian_dje: Keysym = Keysym(0x000006a1);
    pub const Macedonia_gje: Keysym = Keysym(0x000006a2);
    pub const Cyrillic_io: Keysym = Keysym(0x000006a3);
    pub const Ukrainian_ie: Keysym = Keysym(0x000006a4);
    pub const Ukranian_je: Keysym = Keysym(0x000006a4);
    pub const Macedonia_dse: Keysym = Keysym(0x000006a5);
    pub const Ukrainian_i: Keysym = Keysym(0x000006a6);
    pub const Ukranian_i: Keysym = Keysym(0x000006a6);
    pub const Ukrainian_yi: Keysym = Keysym(0x000006a7);
    pub const Ukranian_yi: Keysym = Keysym(0x000006a7);
    pub const Cyrillic_je: Keysym = Keysym(0x000006a8);
    pub const Serbian_je: Keysym = Keysym(0x000006a8);
    pub const Cyrillic_lje: Keysym = Keysym(0x000006a9);
    pub const Serbian_lje: Keysym = Keysym(0x000006a9);
    pub const Cyrillic_nje: Keysym = Keysym(0x000006aa);
    pub const Serbian_nje: Keysym = Keysym(0x000006aa);
    pub const Serbian_tshe: Keysym = Keysym(0x000006ab);
    pub const Macedonia_kje: Keysym = Keysym(0x000006ac);
    pub const Ukrainian_ghe_with_upturn: Keysym = Keysym(0x000006ad);
    pub const Byelorussian_shortu: Keysym = Keysym(0x000006ae);
    pub const Cyrillic_dzhe: Keysym = Keysym(0x000006af);
    pub const Serbian_dze: Keysym = Keysym(0x000006af);
    pub const numerosign: Keysym = Keysym(0x000006b0);
    pub const Serbian_DJE: Keysym = Keysym(0x000006b1);
    pub const Macedonia_GJE: Keysym = Keysym(0x000006b2);
    pub const Cyrillic_IO: Keysym = Keysym(0x000006b3);
    pub const Ukrainian_IE: Keysym = Keysym(0x000006b4);
    pub const Ukranian_JE: Keysym = Keysym(0x000006b4);
    pub const Macedonia_DSE: Keysym = Keysym(0x000006b5);
    pub const Ukrainian_I: Keysym = Keysym(0x000006b6);
    pub const Ukranian_I: Keysym = Keysym(0x000006b6);
    pub const Ukrainian_YI: Keysym = Keysym(0x000006b7);
    pub const Ukranian_YI: Keysym = Keysym(0x000006b7);
    pub const Cyrillic_JE: Keysym = Keysym(0x000006b8);
    pub const Serbian_JE: Keysym = Keysym(0x000006b8);
    pub const Cyrillic_LJE: Keysym = Keysym(0x000006b9);
    pub const Serbian_LJE: Keysym = Keysym(0x000006b9);
    pub const Cyrillic_NJE: Keysym = Keysym(0x000006ba);
    pub const Serbian_NJE: Keysym = Keysym(0x000006ba);
    pub const Serbian_TSHE: Keysym = Keysym(0x000006bb);
    pub const Macedonia_KJE: Keysym = Keysym(0x000006bc);
    pub const Ukrainian_GHE_WITH_UPTURN: Keysym = Keysym(0x000006bd);
    pub const Byelorussian_SHORTU: Keysym = Keysym(0x000006be);
    pub const Cyrillic_DZHE: Keysym = Keysym(0x000006bf);
    pub const Serbian_DZE: Keysym = Keysym(0x000006bf);
    pub const Cyrillic_yu: Keysym = Keysym(0x000006c0);
    pub const Cyrillic_a: Keysym = Keysym(0x000006c1);
    pub const Cyrillic_be: Keysym = Keysym(0x000006c2);
    pub const Cyrillic_tse: Keysym = Keysym(0x000006c3);
    pub const Cyrillic_de: Keysym = Keysym(0x000006c4);
    pub const Cyrillic_ie: Keysym = Keysym(0x000006c5);
    pub const Cyrillic_ef: Keysym = Keysym(0x000006c6);
    pub const Cyrillic_ghe: Keysym = Keysym(0x000006c7);
    pub const Cyrillic_ha: Keysym = Keysym(0x000006c8);
    pub const Cyrillic_i: Keysym = Keysym(0x000006c9);
    pub const Cyrillic_shorti: Keysym = Keysym(0x000006ca);
    pub const Cyrillic_ka: Keysym = Keysym(0x000006cb);
    pub const Cyrillic_el: Keysym = Keysym(0x000006cc);
    pub const Cyrillic_em: Keysym = Keysym(0x000006cd);
    pub const Cyrillic_en: Keysym = Keysym(0x000006ce);
    pub const Cyrillic_o: Keysym = Keysym(0x000006cf);
    pub const Cyrillic_pe: Keysym = Keysym(0x000006d0);
    pub const Cyrillic_ya: Keysym = Keysym(0x000006d1);
    pub const Cyrillic_er: Keysym = Keysym(0x000006d2);
    pub const Cyrillic_es: Keysym = Keysym(0x000006d3);
    pub const Cyrillic_te: Keysym = Keysym(0x000006d4);
    pub const Cyrillic_u: Keysym = Keysym(0x000006d5);
    pub const Cyrillic_zhe: Keysym = Keysym(0x000006d6);
    pub const Cyrillic_ve: Keysym = Keysym(0x000006d7);
    pub const Cyrillic_softsign: Keysym = Keysym(0x000006d8);
    pub const Cyrillic_yeru: Keysym = Keysym(0x000006d9);
    pub const Cyrillic_ze: Keysym = Keysym(0x000006da);
    pub const Cyrillic_sha: Keysym = Keysym(0x000006db);
    pub const Cyrillic_e: Keysym = Keysym(0x000006dc);
    pub const Cyrillic_shcha: Keysym = Keysym(0x000006dd);
    pub const Cyrillic_che: Keysym = Keysym(0x000006de);
    pub const Cyrillic_hardsign: Keysym = Keysym(0x000006df);
    pub const Cyrillic_YU: Keysym = Keysym(0x000006e0);
    pub const Cyrillic_A: Keysym = Keysym(0x000006e1);
    pub const Cyrillic_BE: Keysym = Keysym(0x000006e2);
    pub const Cyrillic_TSE: Keysym = Keysym(0x000006e3);
    pub const Cyrillic_DE: Keysym = Keysym(0x000006e4);
    pub const Cyrillic_IE: Keysym = Keysym(0x000006e5);
    pub const Cyrillic_EF: Keysym = Keysym(0x000006e6);
    pub const Cyrillic_GHE: Keysym = Keysym(0x000006e7);
    pub const Cyrillic_HA: Keysym = Keysym(0x000006e8);
    pub const Cyrillic_I: Keysym = Keysym(0x000006e9);
    pub const Cyrillic_SHORTI: Keysym = Keysym(0x000006ea);
    pub const Cyrillic_KA: Keysym = Keysym(0x000006eb);
    pub const Cyrillic_EL: Keysym = Keysym(0x000006ec);
    pub const Cyrillic_EM: Keysym = Keysym(0x000006ed);
    pub const Cyrillic_EN: Keysym = Keysym(0x000006ee);
    pub const Cyrillic_O: Keysym = Keysym(0x000006ef);
    pub const Cyrillic_PE: Keysym = Keysym(0x000006f0);
    pub const Cyrillic_YA: Keysym = Keysym(0x000006f1);
    pub const Cyrillic_ER: Keysym = Keysym(0x000006f2);
    pub const Cyrillic_ES: Keysym = Keysym(0x000006f3);
    pub const Cyrillic_TE: Keysym = Keysym(0x000006f4);
    pub const Cyrillic_U: Keysym = Keysym(0x000006f5);
    pub const Cyrillic_ZHE: Keysym = Keysym(0x000006f6);
    pub const Cyrillic_VE: Keysym = Keysym(0x000006f7);
    pub const Cyrillic_SOFTSIGN: Keysym = Keysym(0x000006f8);
    pub const Cyrillic_YERU: Keysym = Keysym(0x000006f9);
    pub const Cyrillic_ZE: Keysym = Keysym(0x000006fa);
    pub const Cyrillic_SHA: Keysym = Keysym(0x000006fb);
    pub const Cyrillic_E: Keysym = Keysym(0x000006fc);
    pub const Cyrillic_SHCHA: Keysym = Keysym(0x000006fd);
    pub const Cyrillic_CHE: Keysym = Keysym(0x000006fe);
    pub const Cyrillic_HARDSIGN: Keysym = Keysym(0x000006ff);
    pub const Greek_ALPHAaccent: Keysym = Keysym(0x000007a1);
    pub const Greek_EPSILONaccent: Keysym = Keysym(0x000007a2);
    pub const Greek_ETAaccent: Keysym = Keysym(0x000007a3);
    pub const Greek_IOTAaccent: Keysym = Keysym(0x000007a4);
    pub const Greek_IOTAdieresis: Keysym = Keysym(0x000007a5);
    pub const Greek_IOTAdiaeresis: Keysym = Keysym(0x000007a5);
    pub const Greek_OMICRONaccent: Keysym = Keysym(0x000007a7);
    pub const Greek_UPSILONaccent: Keysym = Keysym(0x000007a8);
    pub const Greek_UPSILONdieresis: Keysym = Keysym(0x000007a9);
    pub const Greek_OMEGAaccent: Keysym = Keysym(0x000007ab);
    pub const Greek_accentdieresis: Keysym = Keysym(0x000007ae);
    pub const Greek_horizbar: Keysym = Keysym(0x000007af);
    pub const Greek_alphaaccent: Keysym = Keysym(0x000007b1);
    pub const Greek_epsilonaccent: Keysym = Keysym(0x000007b2);
    pub const Greek_etaaccent: Keysym = Keysym(0x000007b3);
    pub const Greek_iotaaccent: Keysym = Keysym(0x000007b4);
    pub const Greek_iotadieresis: Keysym = Keysym(0x000007b5);
    pub const Greek_iotaaccentdieresis: Keysym = Keysym(0x000007b6);
    pub const Greek_omicronaccent: Keysym = Keysym(0x000007b7);
    pub const Greek_upsilonaccent: Keysym = Keysym(0x000007b8);
    pub const Greek_upsilondieresis: Keysym = Keysym(0x000007b9);
    pub const Greek_upsilonaccentdieresis: Keysym = Keysym(0x000007ba);
    pub const Greek_omegaaccent: Keysym = Keysym(0x000007bb);
    pub const Greek_ALPHA: Keysym = Keysym(0x000007c1);
    pub const Greek_BETA: Keysym = Keysym(0x000007c2);
    pub const Greek_GAMMA: Keysym = Keysym(0x000007c3);
    pub const Greek_DELTA: Keysym = Keysym(0x000007c4);
    pub const Greek_EPSILON: Keysym = Keysym(0x000007c5);
    pub const Greek_ZETA: Keysym = Keysym(0x000007c6);
    pub const Greek_ETA: Keysym = Keysym(0x000007c7);
    pub const Greek_THETA: Keysym = Keysym(0x000007c8);
    pub const Greek_IOTA: Keysym = Keysym(0x000007c9);
    pub const Greek_KAPPA: Keysym = Keysym(0x000007ca);
    pub const Greek_LAMDA: Keysym = Keysym(0x000007cb);
    pub const Greek_LAMBDA: Keysym = Keysym(0x000007cb);
    pub const Greek_MU: Keysym = Keysym(0x000007cc);
    pub const Greek_NU: Keysym = Keysym(0x000007cd);
    pub const Greek_XI: Keysym = Keysym(0x000007ce);
    pub const Greek_OMICRON: Keysym = Keysym(0x000007cf);
    pub const Greek_PI: Keysym = Keysym(0x000007d0);
    pub const Greek_RHO: Keysym = Keysym(0x000007d1);
    pub const Greek_SIGMA: Keysym = Keysym(0x000007d2);
    pub const Greek_TAU: Keysym = Keysym(0x000007d4);
    pub const Greek_UPSILON: Keysym = Keysym(0x000007d5);
    pub const Greek_PHI: Keysym = Keysym(0x000007d6);
    pub const Greek_CHI: Keysym = Keysym(0x000007d7);
    pub const Greek_PSI: Keysym = Keysym(0x000007d8);
    pub const Greek_OMEGA: Keysym = Keysym(0x000007d9);
    pub const Greek_alpha: Keysym = Keysym(0x000007e1);
    pub const Greek_beta: Keysym = Keysym(0x000007e2);
    pub const Greek_gamma: Keysym = Keysym(0x000007e3);
    pub const Greek_delta: Keysym = Keysym(0x000007e4);
    pub const Greek_epsilon: Keysym = Keysym(0x000007e5);
    pub const Greek_zeta: Keysym = Keysym(0x000007e6);
    pub const Greek_eta: Keysym = Keysym(0x000007e7);
    pub const Greek_theta: Keysym = Keysym(0x000007e8);
    pub const Greek_iota: Keysym = Keysym(0x000007e9);
    pub const Greek_kappa: Keysym = Keysym(0x000007ea);
    pub const Greek_lamda: Keysym = Keysym(0x000007eb);
    pub const Greek_lambda: Keysym = Keysym(0x000007eb);
    pub const Greek_mu: Keysym = Keysym(0x000007ec);
    pub const Greek_nu: Keysym = Keysym(0x000007ed);
    pub const Greek_xi: Keysym = Keysym(0x000007ee);
    pub const Greek_omicron: Keysym = Keysym(0x000007ef);
    pub const Greek_pi: Keysym = Keysym(0x000007f0);
    pub const Greek_rho: Keysym = Keysym(0x000007f1);
    pub const Greek_sigma: Keysym = Keysym(0x000007f2);
    pub const Greek_finalsmallsigma: Keysym = Keysym(0x000007f3);
    pub const Greek_tau: Keysym = Keysym(0x000007f4);
    pub const Greek_upsilon: Keysym = Keysym(0x000007f5);
    pub const Greek_phi: Keysym = Keysym(0x000007f6);
    pub const Greek_chi: Keysym = Keysym(0x000007f7);
    pub const Greek_psi: Keysym = Keysym(0x000007f8);
    pub const Greek_omega: Keysym = Keysym(0x000007f9);
    pub const leftradical: Keysym = Keysym(0x000008a1);
    pub const topleftradical: Keysym = Keysym(0x000008a2);
    pub const horizconnector: Keysym = Keysym(0x000008a3);
    pub const topintegral: Keysym = Keysym(0x000008a4);
    pub const botintegral: Keysym = Keysym(0x000008a5);
    pub const vertconnector: Keysym = Keysym(0x000008a6);
    pub const topleftsqbracket: Keysym = Keysym(0x000008a7);
    pub const botleftsqbracket: Keysym = Keysym(0x000008a8);
    pub const toprightsqbracket: Keysym = Keysym(0x000008a9);
    pub const botrightsqbracket: Keysym = Keysym(0x000008aa);
    pub const topleftparens: Keysym = Keysym(0x000008ab);
    pub const botleftparens: Keysym = Keysym(0x000008ac);
    pub const toprightparens: Keysym = Keysym(0x000008ad);
    pub const botrightparens: Keysym = Keysym(0x000008ae);
    pub const leftmiddlecurlybrace: Keysym = Keysym(0x000008af);
    pub const rightmiddlecurlybrace: Keysym = Keysym(0x000008b0);
    pub const topleftsummation: Keysym = Keysym(0x000008b1);
    pub const botleftsummation: Keysym = Keysym(0x000008b2);
    pub const topvertsummationconnector: Keysym = Keysym(0x000008b3);
    pub const botvertsummationconnector: Keysym = Keysym(0x000008b4);
    pub const toprightsummation: Keysym = Keysym(0x000008b5);
    pub const botrightsummation: Keysym = Keysym(0x000008b6);
    pub const rightmiddlesummation: Keysym = Keysym(0x000008b7);
    pub const lessthanequal: Keysym = Keysym(0x000008bc);
    pub const notequal: Keysym = Keysym(0x000008bd);
    pub const greaterthanequal: Keysym = Keysym(0x000008be);
    pub const integral: Keysym = Keysym(0x000008bf);
    pub const therefore: Keysym = Keysym(0x000008c0);
    pub const variation: Keysym = Keysym(0x000008c1);
    pub const infinity: Keysym = Keysym(0x000008c2);
    pub const nabla: Keysym = Keysym(0x000008c5);
    pub const approximate: Keysym = Keysym(0x000008c8);
    pub const similarequal: Keysym = Keysym(0x000008c9);
    pub const ifonlyif: Keysym = Keysym(0x000008cd);
    pub const implies: Keysym = Keysym(0x000008ce);
    pub const identical: Keysym = Keysym(0x000008cf);
    pub const radical: Keysym = Keysym(0x000008d6);
    pub const includedin: Keysym = Keysym(0x000008da);
    pub const includes: Keysym = Keysym(0x000008db);
    pub const intersection: Keysym = Keysym(0x000008dc);
    pub const union: Keysym = Keysym(0x000008dd);
    pub const logicaland: Keysym = Keysym(0x000008de);
    pub const logicalor: Keysym = Keysym(0x000008df);
    pub const partialderivative: Keysym = Keysym(0x000008ef);
    pub const function: Keysym = Keysym(0x000008f6);
    pub const leftarrow: Keysym = Keysym(0x000008fb);
    pub const uparrow: Keysym = Keysym(0x000008fc);
    pub const rightarrow: Keysym = Keysym(0x000008fd);
    pub const downarrow: Keysym = Keysym(0x000008fe);
    pub const blank: Keysym = Keysym(0x000009df);
    pub const soliddiamond: Keysym = Keysym(0x000009e0);
    pub const checkerboard: Keysym = Keysym(0x000009e1);
    pub const ht: Keysym = Keysym(0x000009e2);
    pub const ff: Keysym = Keysym(0x000009e3);
    pub const cr: Keysym = Keysym(0x000009e4);
    pub const lf: Keysym = Keysym(0x000009e5);
    pub const nl: Keysym = Keysym(0x000009e8);
    pub const vt: Keysym = Keysym(0x000009e9);
    pub const lowrightcorner: Keysym = Keysym(0x000009ea);
    pub const uprightcorner: Keysym = Keysym(0x000009eb);
    pub const upleftcorner: Keysym = Keysym(0x000009ec);
    pub const lowleftcorner: Keysym = Keysym(0x000009ed);
    pub const crossinglines: Keysym = Keysym(0x000009ee);
    pub const horizlinescan1: Keysym = Keysym(0x000009ef);
    pub const horizlinescan3: Keysym = Keysym(0x000009f0);
    pub const horizlinescan5: Keysym = Keysym(0x000009f1);
    pub const horizlinescan7: Keysym = Keysym(0x000009f2);
    pub const horizlinescan9: Keysym = Keysym(0x000009f3);
    pub const leftt: Keysym = Keysym(0x000009f4);
    pub const rightt: Keysym = Keysym(0x000009f5);
    pub const bott: Keysym = Keysym(0x000009f6);
    pub const topt: Keysym = Keysym(0x000009f7);
    pub const vertbar: Keysym = Keysym(0x000009f8);
    pub const emspace: Keysym = Keysym(0x00000aa1);
    pub const enspace: Keysym = Keysym(0x00000aa2);
    pub const em3space: Keysym = Keysym(0x00000aa3);
    pub const em4space: Keysym = Keysym(0x00000aa4);
    pub const digitspace: Keysym = Keysym(0x00000aa5);
    pub const punctspace: Keysym = Keysym(0x00000aa6);
    pub const thinspace: Keysym = Keysym(0x00000aa7);
    pub const hairspace: Keysym = Keysym(0x00000aa8);
    pub const emdash: Keysym = Keysym(0x00000aa9);
    pub const endash: Keysym = Keysym(0x00000aaa);
    pub const signifblank: Keysym = Keysym(0x00000aac);
    pub const ellipsis: Keysym = Keysym(0x00000aae);
    pub const doubbaselinedot: Keysym = Keysym(0x00000aaf);
    pub const onethird: Keysym = Keysym(0x00000ab0);
    pub const twothirds: Keysym = Keysym(0x00000ab1);
    pub const onefifth: Keysym = Keysym(0x00000ab2);
    pub const twofifths: Keysym = Keysym(0x00000ab3);
    pub const threefifths: Keysym = Keysym(0x00000ab4);
    pub const fourfifths: Keysym = Keysym(0x00000ab5);
    pub const onesixth: Keysym = Keysym(0x00000ab6);
    pub const fivesixths: Keysym = Keysym(0x00000ab7);
    pub const careof: Keysym = Keysym(0x00000ab8);
    pub const figdash: Keysym = Keysym(0x00000abb);
    pub const leftanglebracket: Keysym = Keysym(0x00000abc);
    pub const decimalpoint: Keysym = Keysym(0x00000abd);
    pub const rightanglebracket: Keysym = Keysym(0x00000abe);
    pub const marker: Keysym = Keysym(0x00000abf);
    pub const oneeighth: Keysym = Keysym(0x00000ac3);
    pub const threeeighths: Keysym = Keysym(0x00000ac4);
    pub const fiveeighths: Keysym = Keysym(0x00000ac5);
    pub const seveneighths: Keysym = Keysym(0x00000ac6);
    pub const trademark: Keysym = Keysym(0x00000ac9);
    pub const signaturemark: Keysym = Keysym(0x00000aca);
    pub const trademarkincircle: Keysym = Keysym(0x00000acb);
    pub const leftopentriangle: Keysym = Keysym(0x00000acc);
    pub const rightopentriangle: Keysym = Keysym(0x00000acd);
    pub const emopencircle: Keysym = Keysym(0x00000ace);
    pub const emopenrectangle: Keysym = Keysym(0x00000acf);
    pub const leftsinglequotemark: Keysym = Keysym(0x00000ad0);
    pub const rightsinglequotemark: Keysym = Keysym(0x00000ad1);
    pub const leftdoublequotemark: Keysym = Keysym(0x00000ad2);
    pub const rightdoublequotemark: Keysym = Keysym(0x00000ad3);
    pub const prescription: Keysym = Keysym(0x00000ad4);
    pub const permille: Keysym = Keysym(0x00000ad5);
    pub const minutes: Keysym = Keysym(0x00000ad6);
    pub const seconds: Keysym = Keysym(0x00000ad7);
    pub const latincross: Keysym = Keysym(0x00000ad9);
    pub const hexagram: Keysym = Keysym(0x00000ada);
    pub const filledrectbullet: Keysym = Keysym(0x00000adb);
    pub const filledlefttribullet: Keysym = Keysym(0x00000adc);
    pub const filledrighttribullet: Keysym = Keysym(0x00000add);
    pub const emfilledcircle: Keysym = Keysym(0x00000ade);
    pub const emfilledrect: Keysym = Keysym(0x00000adf);
    pub const enopencircbullet: Keysym = Keysym(0x00000ae0);
    pub const enopensquarebullet: Keysym = Keysym(0x00000ae1);
    pub const openrectbullet: Keysym = Keysym(0x00000ae2);
    pub const opentribulletup: Keysym = Keysym(0x00000ae3);
    pub const opentribulletdown: Keysym = Keysym(0x00000ae4);
    pub const openstar: Keysym = Keysym(0x00000ae5);
    pub const enfilledcircbullet: Keysym = Keysym(0x00000ae6);
    pub const enfilledsqbullet: Keysym = Keysym(0x00000ae7);
    pub const filledtribulletup: Keysym = Keysym(0x00000ae8);
    pub const filledtribulletdown: Keysym = Keysym(0x00000ae9);
    pub const leftpointer: Keysym = Keysym(0x00000aea);
    pub const rightpointer: Keysym = Keysym(0x00000aeb);
    pub const club: Keysym = Keysym(0x00000aec);
    pub const diamond: Keysym = Keysym(0x00000aed);
    pub const heart: Keysym = Keysym(0x00000aee);
    pub const maltesecross: Keysym = Keysym(0x00000af0);
    pub const dagger: Keysym = Keysym(0x00000af1);
    pub const doubledagger: Keysym = Keysym(0x00000af2);
    pub const checkmark: Keysym = Keysym(0x00000af3);
    pub const ballotcross: Keysym = Keysym(0x00000af4);
    pub const musicalsharp: Keysym = Keysym(0x00000af5);
    pub const musicalflat: Keysym = Keysym(0x00000af6);
    pub const malesymbol: Keysym = Keysym(0x00000af7);
    pub const femalesymbol: Keysym = Keysym(0x00000af8);
    pub const telephone: Keysym = Keysym(0x00000af9);
    pub const telephonerecorder: Keysym = Keysym(0x00000afa);
    pub const phonographcopyright: Keysym = Keysym(0x00000afb);
    pub const caret: Keysym = Keysym(0x00000afc);
    pub const singlelowquotemark: Keysym = Keysym(0x00000afd);
    pub const doublelowquotemark: Keysym = Keysym(0x00000afe);
    pub const cursor: Keysym = Keysym(0x00000aff);
    pub const leftcaret: Keysym = Keysym(0x00000ba3);
    pub const rightcaret: Keysym = Keysym(0x00000ba6);
    pub const downcaret: Keysym = Keysym(0x00000ba8);
    pub const upcaret: Keysym = Keysym(0x00000ba9);
    pub const overbar: Keysym = Keysym(0x00000bc0);
    pub const downtack: Keysym = Keysym(0x00000bc2);
    pub const upshoe: Keysym = Keysym(0x00000bc3);
    pub const downstile: Keysym = Keysym(0x00000bc4);
    pub const underbar: Keysym = Keysym(0x00000bc6);
    pub const jot: Keysym = Keysym(0x00000bca);
    pub const quad: Keysym = Keysym(0x00000bcc);
    pub const uptack: Keysym = Keysym(0x00000bce);
    pub const circle: Keysym = Keysym(0x00000bcf);
    pub const upstile: Keysym = Keysym(0x00000bd3);
    pub const downshoe: Keysym = Keysym(0x00000bd6);
    pub const rightshoe: Keysym = Keysym(0x00000bd8);
    pub const leftshoe: Keysym = Keysym(0x00000bda);
    pub const lefttack: Keysym = Keysym(0x00000bdc);
    pub const righttack: Keysym = Keysym(0x00000bfc);
    pub const hebrew_doublelowline: Keysym = Keysym(0x00000cdf);
    pub const hebrew_aleph: Keysym = Keysym(0x00000ce0);
    pub const hebrew_bet: Keysym = Keysym(0x00000ce1);
    pub const hebrew_beth: Keysym = Keysym(0x00000ce1);
    pub const hebrew_gimel: Keysym = Keysym(0x00000ce2);
    pub const hebrew_gimmel: Keysym = Keysym(0x00000ce2);
    pub const hebrew_dalet: Keysym = Keysym(0x00000ce3);
    pub const hebrew_daleth: Keysym = Keysym(0x00000ce3);
    pub const hebrew_he: Keysym = Keysym(0x00000ce4);
    pub const hebrew_waw: Keysym = Keysym(0x00000ce5);
    pub const hebrew_zain: Keysym = Keysym(0x00000ce6);
    pub const hebrew_zayin: Keysym = Keysym(0x00000ce6);
    pub const hebrew_chet: Keysym = Keysym(0x00000ce7);
    pub const hebrew_het: Keysym = Keysym(0x00000ce7);
    pub const hebrew_tet: Keysym = Keysym(0x00000ce8);
    pub const hebrew_teth: Keysym = Keysym(0x00000ce8);
    pub const hebrew_yod: Keysym = Keysym(0x00000ce9);
    pub const hebrew_finalkaph: Keysym = Keysym(0x00000cea);
    pub const hebrew_kaph: Keysym = Keysym(0x00000ceb);
    pub const hebrew_lamed: Keysym = Keysym(0x00000cec);
    pub const hebrew_finalmem: Keysym = Keysym(0x00000ced);
    pub const hebrew_mem: Keysym = Keysym(0x00000cee);
    pub const hebrew_finalnun: Keysym = Keysym(0x00000cef);
    pub const hebrew_nun: Keysym = Keysym(0x00000cf0);
    pub const hebrew_samech: Keysym = Keysym(0x00000cf1);
    pub const hebrew_samekh: Keysym = Keysym(0x00000cf1);
    pub const hebrew_ayin: Keysym = Keysym(0x00000cf2);
    pub const hebrew_finalpe: Keysym = Keysym(0x00000cf3);
    pub const hebrew_pe: Keysym = Keysym(0x00000cf4);
    pub const hebrew_finalzade: Keysym = Keysym(0x00000cf5);
    pub const hebrew_finalzadi: Keysym = Keysym(0x00000cf5);
    pub const hebrew_zade: Keysym = Keysym(0x00000cf6);
    pub const hebrew_zadi: Keysym = Keysym(0x00000cf6);
    pub const hebrew_qoph: Keysym = Keysym(0x00000cf7);
    pub const hebrew_kuf: Keysym = Keysym(0x00000cf7);
    pub const hebrew_resh: Keysym = Keysym(0x00000cf8);
    pub const hebrew_shin: Keysym = Keysym(0x00000cf9);
    pub const hebrew_taw: Keysym = Keysym(0x00000cfa);
    pub const hebrew_taf: Keysym = Keysym(0x00000cfa);
    pub const Thai_kokai: Keysym = Keysym(0x00000da1);
    pub const Thai_khokhai: Keysym = Keysym(0x00000da2);
    pub const Thai_khokhuat: Keysym = Keysym(0x00000da3);
    pub const Thai_khokhwai: Keysym = Keysym(0x00000da4);
    pub const Thai_khokhon: Keysym = Keysym(0x00000da5);
    pub const Thai_khorakhang: Keysym = Keysym(0x00000da6);
    pub const Thai_ngongu: Keysym = Keysym(0x00000da7);
    pub const Thai_chochan: Keysym = Keysym(0x00000da8);
    pub const Thai_choching: Keysym = Keysym(0x00000da9);
    pub const Thai_chochang: Keysym = Keysym(0x00000daa);
    pub const Thai_soso: Keysym = Keysym(0x00000dab);
    pub const Thai_chochoe: Keysym = Keysym(0x00000dac);
    pub const Thai_yoying: Keysym = Keysym(0x00000dad);
    pub const Thai_dochada: Keysym = Keysym(0x00000dae);
    pub const Thai_topatak: Keysym = Keysym(0x00000daf);
    pub const Thai_thothan: Keysym = Keysym(0x00000db0);
    pub const Thai_thonangmontho: Keysym = Keysym(0x00000db1);
    pub const Thai_thophuthao: Keysym = Keysym(0x00000db2);
    pub const Thai_nonen: Keysym = Keysym(0x00000db3);
    pub const Thai_dodek: Keysym = Keysym(0x00000db4);
    pub const Thai_totao: Keysym = Keysym(0x00000db5);
    pub const Thai_thothung: Keysym = Keysym(0x00000db6);
    pub const Thai_thothahan: Keysym = Keysym(0x00000db7);
    pub const Thai_thothong: Keysym = Keysym(0x00000db8);
    pub const Thai_nonu: Keysym = Keysym(0x00000db9);
    pub const Thai_bobaimai: Keysym = Keysym(0x00000dba);
    pub const Thai_popla: Keysym = Keysym(0x00000dbb);
    pub const Thai_phophung: Keysym = Keysym(0x00000dbc);
    pub const Thai_fofa: Keysym = Keysym(0x00000dbd);
    pub const Thai_phophan: Keysym = Keysym(0x00000dbe);
    pub const Thai_fofan: Keysym = Keysym(0x00000dbf);
    pub const Thai_phosamphao: Keysym = Keysym(0x00000dc0);
    pub const Thai_moma: Keysym = Keysym(0x00000dc1);
    pub const Thai_yoyak: Keysym = Keysym(0x00000dc2);
    pub const Thai_rorua: Keysym = Keysym(0x00000dc3);
    pub const Thai_ru: Keysym = Keysym(0x00000dc4);
    pub const Thai_loling: Keysym = Keysym(0x00000dc5);
    pub const Thai_lu: Keysym = Keysym(0x00000dc6);
    pub const Thai_wowaen: Keysym = Keysym(0x00000dc7);
    pub const Thai_sosala: Keysym = Keysym(0x00000dc8);
    pub const Thai_sorusi: Keysym = Keysym(0x00000dc9);
    pub const Thai_sosua: Keysym = Keysym(0x00000dca);
    pub const Thai_hohip: Keysym = Keysym(0x00000dcb);
    pub const Thai_lochula: Keysym = Keysym(0x00000dcc);
    pub const Thai_oang: Keysym = Keysym(0x00000dcd);
    pub const Thai_honokhuk: Keysym = Keysym(0x00000dce);
    pub const Thai_paiyannoi: Keysym = Keysym(0x00000dcf);
    pub const Thai_saraa: Keysym = Keysym(0x00000dd0);
    pub const Thai_maihanakat: Keysym = Keysym(0x00000dd1);
    pub const Thai_saraaa: Keysym = Keysym(0x00000dd2);
    pub const Thai_saraam: Keysym = Keysym(0x00000dd3);
    pub const Thai_sarai: Keysym = Keysym(0x00000dd4);
    pub const Thai_saraii: Keysym = Keysym(0x00000dd5);
    pub const Thai_saraue: Keysym = Keysym(0x00000dd6);
    pub const Thai_sarauee: Keysym = Keysym(0x00000dd7);
    pub const Thai_sarau: Keysym = Keysym(0x00000dd8);
    pub const Thai_sarauu: Keysym = Keysym(0x00000dd9);
    pub const Thai_phinthu: Keysym = Keysym(0x00000dda);
    pub const Thai_maihanakat_maitho: Keysym = Keysym(0x00000dde);
    pub const Thai_baht: Keysym = Keysym(0x00000ddf);
    pub const Thai_sarae: Keysym = Keysym(0x00000de0);
    pub const Thai_saraae: Keysym = Keysym(0x00000de1);
    pub const Thai_sarao: Keysym = Keysym(0x00000de2);
    pub const Thai_saraaimaimuan: Keysym = Keysym(0x00000de3);
    pub const Thai_saraaimaimalai: Keysym = Keysym(0x00000de4);
    pub const Thai_lakkhangyao: Keysym = Keysym(0x00000de5);
    pub const Thai_maiyamok: Keysym = Keysym(0x00000de6);
    pub const Thai_maitaikhu: Keysym = Keysym(0x00000de7);
    pub const Thai_maiek: Keysym = Keysym(0x00000de8);
    pub const Thai_maitho: Keysym = Keysym(0x00000de9);
    pub const Thai_maitri: Keysym = Keysym(0x00000dea);
    pub const Thai_maichattawa: Keysym = Keysym(0x00000deb);
    pub const Thai_thanthakhat: Keysym = Keysym(0x00000dec);
    pub const Thai_nikhahit: Keysym = Keysym(0x00000ded);
    pub const Thai_leksun: Keysym = Keysym(0x00000df0);
    pub const Thai_leknung: Keysym = Keysym(0x00000df1);
    pub const Thai_leksong: Keysym = Keysym(0x00000df2);
    pub const Thai_leksam: Keysym = Keysym(0x00000df3);
    pub const Thai_leksi: Keysym = Keysym(0x00000df4);
    pub const Thai_lekha: Keysym = Keysym(0x00000df5);
    pub const Thai_lekhok: Keysym = Keysym(0x00000df6);
    pub const Thai_lekchet: Keysym = Keysym(0x00000df7);
    pub const Thai_lekpaet: Keysym = Keysym(0x00000df8);
    pub const Thai_lekkao: Keysym = Keysym(0x00000df9);
    pub const Hangul_Kiyeog: Keysym = Keysym(0x00000ea1);
    pub const Hangul_SsangKiyeog: Keysym = Keysym(0x00000ea2);
    pub const Hangul_KiyeogSios: Keysym = Keysym(0x00000ea3);
    pub const Hangul_Nieun: Keysym = Keysym(0x00000ea4);
    pub const Hangul_NieunJieuj: Keysym = Keysym(0x00000ea5);
    pub const Hangul_NieunHieuh: Keysym = Keysym(0x00000ea6);
    pub const Hangul_Dikeud: Keysym = Keysym(0x00000ea7);
    pub const Hangul_SsangDikeud: Keysym = Keysym(0x00000ea8);
    pub const Hangul_Rieul: Keysym = Keysym(0x00000ea9);
    pub const Hangul_RieulKiyeog: Keysym = Keysym(0x00000eaa);
    pub const Hangul_RieulMieum: Keysym = Keysym(0x00000eab);
    pub const Hangul_RieulPieub: Keysym = Keysym(0x00000eac);
    pub const Hangul_RieulSios: Keysym = Keysym(0x00000ead);
    pub const Hangul_RieulTieut: Keysym = Keysym(0x00000eae);
    pub const Hangul_RieulPhieuf: Keysym = Keysym(0x00000eaf);
    pub const Hangul_RieulHieuh: Keysym = Keysym(0x00000eb0);
    pub const Hangul_Mieum: Keysym = Keysym(0x00000eb1);
    pub const Hangul_Pieub: Keysym = Keysym(0x00000eb2);
    pub const Hangul_SsangPieub: Keysym = Keysym(0x00000eb3);
    pub const Hangul_PieubSios: Keysym = Keysym(0x00000eb4);
    pub const Hangul_Sios: Keysym = Keysym(0x00000eb5);
    pub const Hangul_SsangSios: Keysym = Keysym(0x00000eb6);
    pub const Hangul_Ieung: Keysym = Keysym(0x00000eb7);
    pub const Hangul_Jieuj: Keysym = Keysym(0x00000eb8);
    pub const Hangul_SsangJieuj: Keysym = Keysym(0x00000eb9);
    pub const Hangul_Cieuc: Keysym = Keysym(0x00000eba);
    pub const Hangul_Khieuq: Keysym = Keysym(0x00000ebb);
    pub const Hangul_Tieut: Keysym = Keysym(0x00000ebc);
    pub const Hangul_Phieuf: Keysym = Keysym(0x00000ebd);
    pub const Hangul_Hieuh: Keysym = Keysym(0x00000ebe);
    pub const Hangul_A: Keysym = Keysym(0x00000ebf);
    pub const Hangul_AE: Keysym = Keysym(0x00000ec0);
    pub const Hangul_YA: Keysym = Keysym(0x00000ec1);
    pub const Hangul_YAE: Keysym = Keysym(0x00000ec2);
    pub const Hangul_EO: Keysym = Keysym(0x00000ec3);
    pub const Hangul_E: Keysym = Keysym(0x00000ec4);
    pub const Hangul_YEO: Keysym = Keysym(0x00000ec5);
    pub const Hangul_YE: Keysym = Keysym(0x00000ec6);
    pub const Hangul_O: Keysym = Keysym(0x00000ec7);
    pub const Hangul_WA: Keysym = Keysym(0x00000ec8);
    pub const Hangul_WAE: Keysym = Keysym(0x00000ec9);
    pub const Hangul_OE: Keysym = Keysym(0x00000eca);
    pub const Hangul_YO: Keysym = Keysym(0x00000ecb);
    pub const Hangul_U: Keysym = Keysym(0x00000ecc);
    pub const Hangul_WEO: Keysym = Keysym(0x00000ecd);
    pub const Hangul_WE: Keysym = Keysym(0x00000ece);
    pub const Hangul_WI: Keysym = Keysym(0x00000ecf);
    pub const Hangul_YU: Keysym = Keysym(0x00000ed0);
    pub const Hangul_EU: Keysym = Keysym(0x00000ed1);
    pub const Hangul_YI: Keysym = Keysym(0x00000ed2);
    pub const Hangul_I: Keysym = Keysym(0x00000ed3);
    pub const Hangul_J_Kiyeog: Keysym = Keysym(0x00000ed4);
    pub const Hangul_J_SsangKiyeog: Keysym = Keysym(0x00000ed5);
    pub const Hangul_J_KiyeogSios: Keysym = Keysym(0x00000ed6);
    pub const Hangul_J_Nieun: Keysym = Keysym(0x00000ed7);
    pub const Hangul_J_NieunJieuj: Keysym = Keysym(0x00000ed8);
    pub const Hangul_J_NieunHieuh: Keysym = Keysym(0x00000ed9);
    pub const Hangul_J_Dikeud: Keysym = Keysym(0x00000eda);
    pub const Hangul_J_Rieul: Keysym = Keysym(0x00000edb);
    pub const Hangul_J_RieulKiyeog: Keysym = Keysym(0x00000edc);
    pub const Hangul_J_RieulMieum: Keysym = Keysym(0x00000edd);
    pub const Hangul_J_RieulPieub: Keysym = Keysym(0x00000ede);
    pub const Hangul_J_RieulSios: Keysym = Keysym(0x00000edf);
    pub const Hangul_J_RieulTieut: Keysym = Keysym(0x00000ee0);
    pub const Hangul_J_RieulPhieuf: Keysym = Keysym(0x00000ee1);
    pub const Hangul_J_RieulHieuh: Keysym = Keysym(0x00000ee2);
    pub const Hangul_J_Mieum: Keysym = Keysym(0x00000ee3);
    pub const Hangul_J_Pieub: Keysym = Keysym(0x00000ee4);
    pub const Hangul_J_PieubSios: Keysym = Keysym(0x00000ee5);
    pub const Hangul_J_Sios: Keysym = Keysym(0x00000ee6);
    pub const Hangul_J_SsangSios: Keysym = Keysym(0x00000ee7);
    pub const Hangul_J_Ieung: Keysym = Keysym(0x00000ee8);
    pub const Hangul_J_Jieuj: Keysym = Keysym(0x00000ee9);
    pub const Hangul_J_Cieuc: Keysym = Keysym(0x00000eea);
    pub const Hangul_J_Khieuq: Keysym = Keysym(0x00000eeb);
    pub const Hangul_J_Tieut: Keysym = Keysym(0x00000eec);
    pub const Hangul_J_Phieuf: Keysym = Keysym(0x00000eed);
    pub const Hangul_J_Hieuh: Keysym = Keysym(0x00000eee);
    pub const Hangul_RieulYeorinHieuh: Keysym = Keysym(0x00000eef);
    pub const Hangul_SunkyeongeumMieum: Keysym = Keysym(0x00000ef0);
    pub const Hangul_SunkyeongeumPieub: Keysym = Keysym(0x00000ef1);
    pub const Hangul_PanSios: Keysym = Keysym(0x00000ef2);
    pub const Hangul_KkogjiDalrinIeung: Keysym = Keysym(0x00000ef3);
    pub const Hangul_SunkyeongeumPhieuf: Keysym = Keysym(0x00000ef4);
    pub const Hangul_YeorinHieuh: Keysym = Keysym(0x00000ef5);
    pub const Hangul_AraeA: Keysym = Keysym(0x00000ef6);
    pub const Hangul_AraeAE: Keysym = Keysym(0x00000ef7);
    pub const Hangul_J_PanSios: Keysym = Keysym(0x00000ef8);
    pub const Hangul_J_KkogjiDalrinIeung: Keysym = Keysym(0x00000ef9);
    pub const Hangul_J_YeorinHieuh: Keysym = Keysym(0x00000efa);
    pub const Korean_Won: Keysym = Keysym(0x00000eff);
    pub const OE: Keysym = Keysym(0x000013bc);
    pub const oe: Keysym = Keysym(0x000013bd);
    pub const Ydiaeresis: Keysym = Keysym(0x000013be);
    pub const EuroSign: Keysym = Keysym(0x000020ac);
    pub const _3270_Duplicate: Keysym = Keysym(0x0000fd01);
    pub const _3270_FieldMark: Keysym = Keysym(0x0000fd02);
    pub const _3270_Right2: Keysym = Keysym(0x0000fd03);
    pub const _3270_Left2: Keysym = Keysym(0x0000fd04);
    pub const _3270_BackTab: Keysym = Keysym(0x0000fd05);
    pub const _3270_EraseEOF: Keysym = Keysym(0x0000fd06);
    pub const _3270_EraseInput: Keysym = Keysym(0x0000fd07);
    pub const _3270_Reset: Keysym = Keysym(0x0000fd08);
    pub const _3270_Quit: Keysym = Keysym(0x0000fd09);
    pub const _3270_PA1: Keysym = Keysym(0x0000fd0a);
    pub const _3270_PA2: Keysym = Keysym(0x0000fd0b);
    pub const _3270_PA3: Keysym = Keysym(0x0000fd0c);
    pub const _3270_Test: Keysym = Keysym(0x0000fd0d);
    pub const _3270_Attn: Keysym = Keysym(0x0000fd0e);
    pub const _3270_CursorBlink: Keysym = Keysym(0x0000fd0f);
    pub const _3270_AltCursor: Keysym = Keysym(0x0000fd10);
    pub const _3270_KeyClick: Keysym = Keysym(0x0000fd11);
    pub const _3270_Jump: Keysym = Keysym(0x0000fd12);
    pub const _3270_Ident: Keysym = Keysym(0x0000fd13);
    pub const _3270_Rule: Keysym = Keysym(0x0000fd14);
    pub const _3270_Copy: Keysym = Keysym(0x0000fd15);
    pub const _3270_Play: Keysym = Keysym(0x0000fd16);
    pub const _3270_Setup: Keysym = Keysym(0x0000fd17);
    pub const _3270_Record: Keysym = Keysym(0x0000fd18);
    pub const _3270_ChangeScreen: Keysym = Keysym(0x0000fd19);
    pub const _3270_DeleteWord: Keysym = Keysym(0x0000fd1a);
    pub const _3270_ExSelect: Keysym = Keysym(0x0000fd1b);
    pub const _3270_CursorSelect: Keysym = Keysym(0x0000fd1c);
    pub const _3270_PrintScreen: Keysym = Keysym(0x0000fd1d);
    pub const _3270_Enter: Keysym = Keysym(0x0000fd1e);
    pub const ISO_Lock: Keysym = Keysym(0x0000fe01);
    pub const ISO_Level2_Latch: Keysym = Keysym(0x0000fe02);
    pub const ISO_Level3_Shift: Keysym = Keysym(0x0000fe03);
    pub const ISO_Level3_Latch: Keysym = Keysym(0x0000fe04);
    pub const ISO_Level3_Lock: Keysym = Keysym(0x0000fe05);
    pub const ISO_Group_Latch: Keysym = Keysym(0x0000fe06);
    pub const ISO_Group_Lock: Keysym = Keysym(0x0000fe07);
    pub const ISO_Next_Group: Keysym = Keysym(0x0000fe08);
    pub const ISO_Next_Group_Lock: Keysym = Keysym(0x0000fe09);
    pub const ISO_Prev_Group: Keysym = Keysym(0x0000fe0a);
    pub const ISO_Prev_Group_Lock: Keysym = Keysym(0x0000fe0b);
    pub const ISO_First_Group: Keysym = Keysym(0x0000fe0c);
    pub const ISO_First_Group_Lock: Keysym = Keysym(0x0000fe0d);
    pub const ISO_Last_Group: Keysym = Keysym(0x0000fe0e);
    pub const ISO_Last_Group_Lock: Keysym = Keysym(0x0000fe0f);
    pub const ISO_Level5_Shift: Keysym = Keysym(0x0000fe11);
    pub const ISO_Level5_Latch: Keysym = Keysym(0x0000fe12);
    pub const ISO_Level5_Lock: Keysym = Keysym(0x0000fe13);
    pub const ISO_Left_Tab: Keysym = Keysym(0x0000fe20);
    pub const ISO_Move_Line_Up: Keysym = Keysym(0x0000fe21);
    pub const ISO_Move_Line_Down: Keysym = Keysym(0x0000fe22);
    pub const ISO_Partial_Line_Up: Keysym = Keysym(0x0000fe23);
    pub const ISO_Partial_Line_Down: Keysym = Keysym(0x0000fe24);
    pub const ISO_Partial_Space_Left: Keysym = Keysym(0x0000fe25);
    pub const ISO_Partial_Space_Right: Keysym = Keysym(0x0000fe26);
    pub const ISO_Set_Margin_Left: Keysym = Keysym(0x0000fe27);
    pub const ISO_Set_Margin_Right: Keysym = Keysym(0x0000fe28);
    pub const ISO_Release_Margin_Left: Keysym = Keysym(0x0000fe29);
    pub const ISO_Release_Margin_Right: Keysym = Keysym(0x0000fe2a);
    pub const ISO_Release_Both_Margins: Keysym = Keysym(0x0000fe2b);
    pub const ISO_Fast_Cursor_Left: Keysym = Keysym(0x0000fe2c);
    pub const ISO_Fast_Cursor_Right: Keysym = Keysym(0x0000fe2d);
    pub const ISO_Fast_Cursor_Up: Keysym = Keysym(0x0000fe2e);
    pub const ISO_Fast_Cursor_Down: Keysym = Keysym(0x0000fe2f);
    pub const ISO_Continuous_Underline: Keysym = Keysym(0x0000fe30);
    pub const ISO_Discontinuous_Underline: Keysym = Keysym(0x0000fe31);
    pub const ISO_Emphasize: Keysym = Keysym(0x0000fe32);
    pub const ISO_Center_Object: Keysym = Keysym(0x0000fe33);
    pub const ISO_Enter: Keysym = Keysym(0x0000fe34);
    pub const dead_grave: Keysym = Keysym(0x0000fe50);
    pub const dead_acute: Keysym = Keysym(0x0000fe51);
    pub const dead_circumflex: Keysym = Keysym(0x0000fe52);
    pub const dead_tilde: Keysym = Keysym(0x0000fe53);
    pub const dead_perispomeni: Keysym = Keysym(0x0000fe53);
    pub const dead_macron: Keysym = Keysym(0x0000fe54);
    pub const dead_breve: Keysym = Keysym(0x0000fe55);
    pub const dead_abovedot: Keysym = Keysym(0x0000fe56);
    pub const dead_diaeresis: Keysym = Keysym(0x0000fe57);
    pub const dead_abovering: Keysym = Keysym(0x0000fe58);
    pub const dead_doubleacute: Keysym = Keysym(0x0000fe59);
    pub const dead_caron: Keysym = Keysym(0x0000fe5a);
    pub const dead_cedilla: Keysym = Keysym(0x0000fe5b);
    pub const dead_ogonek: Keysym = Keysym(0x0000fe5c);
    pub const dead_iota: Keysym = Keysym(0x0000fe5d);
    pub const dead_voiced_sound: Keysym = Keysym(0x0000fe5e);
    pub const dead_semivoiced_sound: Keysym = Keysym(0x0000fe5f);
    pub const dead_belowdot: Keysym = Keysym(0x0000fe60);
    pub const dead_hook: Keysym = Keysym(0x0000fe61);
    pub const dead_horn: Keysym = Keysym(0x0000fe62);
    pub const dead_stroke: Keysym = Keysym(0x0000fe63);
    pub const dead_abovecomma: Keysym = Keysym(0x0000fe64);
    pub const dead_psili: Keysym = Keysym(0x0000fe64);
    pub const dead_abovereversedcomma: Keysym = Keysym(0x0000fe65);
    pub const dead_dasia: Keysym = Keysym(0x0000fe65);
    pub const dead_doublegrave: Keysym = Keysym(0x0000fe66);
    pub const dead_belowring: Keysym = Keysym(0x0000fe67);
    pub const dead_belowmacron: Keysym = Keysym(0x0000fe68);
    pub const dead_belowcircumflex: Keysym = Keysym(0x0000fe69);
    pub const dead_belowtilde: Keysym = Keysym(0x0000fe6a);
    pub const dead_belowbreve: Keysym = Keysym(0x0000fe6b);
    pub const dead_belowdiaeresis: Keysym = Keysym(0x0000fe6c);
    pub const dead_invertedbreve: Keysym = Keysym(0x0000fe6d);
    pub const dead_belowcomma: Keysym = Keysym(0x0000fe6e);
    pub const dead_currency: Keysym = Keysym(0x0000fe6f);
    pub const AccessX_Enable: Keysym = Keysym(0x0000fe70);
    pub const AccessX_Feedback_Enable: Keysym = Keysym(0x0000fe71);
    pub const RepeatKeys_Enable: Keysym = Keysym(0x0000fe72);
    pub const SlowKeys_Enable: Keysym = Keysym(0x0000fe73);
    pub const BounceKeys_Enable: Keysym = Keysym(0x0000fe74);
    pub const StickyKeys_Enable: Keysym = Keysym(0x0000fe75);
    pub const MouseKeys_Enable: Keysym = Keysym(0x0000fe76);
    pub const MouseKeys_Accel_Enable: Keysym = Keysym(0x0000fe77);
    pub const Overlay1_Enable: Keysym = Keysym(0x0000fe78);
    pub const Overlay2_Enable: Keysym = Keysym(0x0000fe79);
    pub const AudibleBell_Enable: Keysym = Keysym(0x0000fe7a);
    pub const dead_a: Keysym = Keysym(0x0000fe80);
    pub const dead_A: Keysym = Keysym(0x0000fe81);
    pub const dead_e: Keysym = Keysym(0x0000fe82);
    pub const dead_E: Keysym = Keysym(0x0000fe83);
    pub const dead_i: Keysym = Keysym(0x0000fe84);
    pub const dead_I: Keysym = Keysym(0x0000fe85);
    pub const dead_o: Keysym = Keysym(0x0000fe86);
    pub const dead_O: Keysym = Keysym(0x0000fe87);
    pub const dead_u: Keysym = Keysym(0x0000fe88);
    pub const dead_U: Keysym = Keysym(0x0000fe89);
    pub const dead_small_schwa: Keysym = Keysym(0x0000fe8a);
    pub const dead_schwa: Keysym = Keysym(0x0000fe8a);
    pub const dead_capital_schwa: Keysym = Keysym(0x0000fe8b);
    pub const dead_SCHWA: Keysym = Keysym(0x0000fe8b);
    pub const dead_greek: Keysym = Keysym(0x0000fe8c);
    pub const dead_hamza: Keysym = Keysym(0x0000fe8d);
    pub const dead_apostrophe: Keysym = Keysym(0x0000fe8e);
    pub const dead_lowline: Keysym = Keysym(0x0000fe90);
    pub const dead_aboveverticalline: Keysym = Keysym(0x0000fe91);
    pub const dead_belowverticalline: Keysym = Keysym(0x0000fe92);
    pub const dead_longsolidusoverlay: Keysym = Keysym(0x0000fe93);
    pub const ch: Keysym = Keysym(0x0000fea0);
    pub const Ch: Keysym = Keysym(0x0000fea1);
    pub const CH: Keysym = Keysym(0x0000fea2);
    pub const c_h: Keysym = Keysym(0x0000fea3);
    pub const C_h: Keysym = Keysym(0x0000fea4);
    pub const C_H: Keysym = Keysym(0x0000fea5);
    pub const First_Virtual_Screen: Keysym = Keysym(0x0000fed0);
    pub const Prev_Virtual_Screen: Keysym = Keysym(0x0000fed1);
    pub const Next_Virtual_Screen: Keysym = Keysym(0x0000fed2);
    pub const Last_Virtual_Screen: Keysym = Keysym(0x0000fed4);
    pub const Terminate_Server: Keysym = Keysym(0x0000fed5);
    pub const Pointer_Left: Keysym = Keysym(0x0000fee0);
    pub const Pointer_Right: Keysym = Keysym(0x0000fee1);
    pub const Pointer_Up: Keysym = Keysym(0x0000fee2);
    pub const Pointer_Down: Keysym = Keysym(0x0000fee3);
    pub const Pointer_UpLeft: Keysym = Keysym(0x0000fee4);
    pub const Pointer_UpRight: Keysym = Keysym(0x0000fee5);
    pub const Pointer_DownLeft: Keysym = Keysym(0x0000fee6);
    pub const Pointer_DownRight: Keysym = Keysym(0x0000fee7);
    pub const Pointer_Button_Dflt: Keysym = Keysym(0x0000fee8);
    pub const Pointer_Button1: Keysym = Keysym(0x0000fee9);
    pub const Pointer_Button2: Keysym = Keysym(0x0000feea);
    pub const Pointer_Button3: Keysym = Keysym(0x0000feeb);
    pub const Pointer_Button4: Keysym = Keysym(0x0000feec);
    pub const Pointer_Button5: Keysym = Keysym(0x0000feed);
    pub const Pointer_DblClick_Dflt: Keysym = Keysym(0x0000feee);
    pub const Pointer_DblClick1: Keysym = Keysym(0x0000feef);
    pub const Pointer_DblClick2: Keysym = Keysym(0x0000fef0);
    pub const Pointer_DblClick3: Keysym = Keysym(0x0000fef1);
    pub const Pointer_DblClick4: Keysym = Keysym(0x0000fef2);
    pub const Pointer_DblClick5: Keysym = Keysym(0x0000fef3);
    pub const Pointer_Drag_Dflt: Keysym = Keysym(0x0000fef4);
    pub const Pointer_Drag1: Keysym = Keysym(0x0000fef5);
    pub const Pointer_Drag2: Keysym = Keysym(0x0000fef6);
    pub const Pointer_Drag3: Keysym = Keysym(0x0000fef7);
    pub const Pointer_Drag4: Keysym = Keysym(0x0000fef8);
    pub const Pointer_EnableKeys: Keysym = Keysym(0x0000fef9);
    pub const Pointer_Accelerate: Keysym = Keysym(0x0000fefa);
    pub const Pointer_DfltBtnNext: Keysym = Keysym(0x0000fefb);
    pub const Pointer_DfltBtnPrev: Keysym = Keysym(0x0000fefc);
    pub const Pointer_Drag5: Keysym = Keysym(0x0000fefd);
    pub const BackSpace: Keysym = Keysym(0x0000ff08);
    pub const Tab: Keysym = Keysym(0x0000ff09);
    pub const Linefeed: Keysym = Keysym(0x0000ff0a);
    pub const Clear: Keysym = Keysym(0x0000ff0b);
    pub const Return: Keysym = Keysym(0x0000ff0d);
    pub const Pause: Keysym = Keysym(0x0000ff13);
    pub const Scroll_Lock: Keysym = Keysym(0x0000ff14);
    pub const Sys_Req: Keysym = Keysym(0x0000ff15);
    pub const Escape: Keysym = Keysym(0x0000ff1b);
    pub const Multi_key: Keysym = Keysym(0x0000ff20);
    pub const SunCompose: Keysym = Keysym(0x0000ff20);
    pub const Kanji: Keysym = Keysym(0x0000ff21);
    pub const Muhenkan: Keysym = Keysym(0x0000ff22);
    pub const Henkan_Mode: Keysym = Keysym(0x0000ff23);
    pub const Henkan: Keysym = Keysym(0x0000ff23);
    pub const Romaji: Keysym = Keysym(0x0000ff24);
    pub const Hiragana: Keysym = Keysym(0x0000ff25);
    pub const Katakana: Keysym = Keysym(0x0000ff26);
    pub const Hiragana_Katakana: Keysym = Keysym(0x0000ff27);
    pub const Zenkaku: Keysym = Keysym(0x0000ff28);
    pub const Hankaku: Keysym = Keysym(0x0000ff29);
    pub const Zenkaku_Hankaku: Keysym = Keysym(0x0000ff2a);
    pub const Touroku: Keysym = Keysym(0x0000ff2b);
    pub const Massyo: Keysym = Keysym(0x0000ff2c);
    pub const Kana_Lock: Keysym = Keysym(0x0000ff2d);
    pub const Kana_Shift: Keysym = Keysym(0x0000ff2e);
    pub const Eisu_Shift: Keysym = Keysym(0x0000ff2f);
    pub const Eisu_toggle: Keysym = Keysym(0x0000ff30);
    pub const Hangul: Keysym = Keysym(0x0000ff31);
    pub const Hangul_Start: Keysym = Keysym(0x0000ff32);
    pub const Hangul_End: Keysym = Keysym(0x0000ff33);
    pub const Hangul_Hanja: Keysym = Keysym(0x0000ff34);
    pub const Hangul_Jamo: Keysym = Keysym(0x0000ff35);
    pub const Hangul_Romaja: Keysym = Keysym(0x0000ff36);
    pub const Codeinput: Keysym = Keysym(0x0000ff37);
    pub const Kanji_Bangou: Keysym = Keysym(0x0000ff37);
    pub const Hangul_Codeinput: Keysym = Keysym(0x0000ff37);
    pub const Hangul_Jeonja: Keysym = Keysym(0x0000ff38);
    pub const Hangul_Banja: Keysym = Keysym(0x0000ff39);
    pub const Hangul_PreHanja: Keysym = Keysym(0x0000ff3a);
    pub const Hangul_PostHanja: Keysym = Keysym(0x0000ff3b);
    pub const SingleCandidate: Keysym = Keysym(0x0000ff3c);
    pub const Hangul_SingleCandidate: Keysym = Keysym(0x0000ff3c);
    pub const MultipleCandidate: Keysym = Keysym(0x0000ff3d);
    pub const Zen_Koho: Keysym = Keysym(0x0000ff3d);
    pub const Hangul_MultipleCandidate: Keysym = Keysym(0x0000ff3d);
    pub const PreviousCandidate: Keysym = Keysym(0x0000ff3e);
    pub const Mae_Koho: Keysym = Keysym(0x0000ff3e);
    pub const Hangul_PreviousCandidate: Keysym = Keysym(0x0000ff3e);
    pub const Hangul_Special: Keysym = Keysym(0x0000ff3f);
    pub const Home: Keysym = Keysym(0x0000ff50);
    pub const Left: Keysym = Keysym(0x0000ff51);
    pub const Up: Keysym = Keysym(0x0000ff52);
    pub const Right: Keysym = Keysym(0x0000ff53);
    pub const Down: Keysym = Keysym(0x0000ff54);
    pub const Prior: Keysym = Keysym(0x0000ff55);
    pub const Page_Up: Keysym = Keysym(0x0000ff55);
    pub const SunPageUp: Keysym = Keysym(0x0000ff55);
    pub const Next: Keysym = Keysym(0x0000ff56);
    pub const Page_Down: Keysym = Keysym(0x0000ff56);
    pub const SunPageDown: Keysym = Keysym(0x0000ff56);
    pub const End: Keysym = Keysym(0x0000ff57);
    pub const Begin: Keysym = Keysym(0x0000ff58);
    pub const Select: Keysym = Keysym(0x0000ff60);
    pub const Print: Keysym = Keysym(0x0000ff61);
    pub const SunPrint_Screen: Keysym = Keysym(0x0000ff61);
    pub const Execute: Keysym = Keysym(0x0000ff62);
    pub const Insert: Keysym = Keysym(0x0000ff63);
    pub const Undo: Keysym = Keysym(0x0000ff65);
    pub const SunUndo: Keysym = Keysym(0x0000ff65);
    pub const Redo: Keysym = Keysym(0x0000ff66);
    pub const SunAgain: Keysym = Keysym(0x0000ff66);
    pub const Menu: Keysym = Keysym(0x0000ff67);
    pub const Find: Keysym = Keysym(0x0000ff68);
    pub const SunFind: Keysym = Keysym(0x0000ff68);
    pub const Cancel: Keysym = Keysym(0x0000ff69);
    pub const SunStop: Keysym = Keysym(0x0000ff69);
    pub const Help: Keysym = Keysym(0x0000ff6a);
    pub const Break: Keysym = Keysym(0x0000ff6b);
    pub const ISO_Group_Shift: Keysym = Keysym(0x0000ff7e);
    pub const Mode_switch: Keysym = Keysym(0x0000ff7e);
    pub const script_switch: Keysym = Keysym(0x0000ff7e);
    pub const kana_switch: Keysym = Keysym(0x0000ff7e);
    pub const Arabic_switch: Keysym = Keysym(0x0000ff7e);
    pub const Greek_switch: Keysym = Keysym(0x0000ff7e);
    pub const Hebrew_switch: Keysym = Keysym(0x0000ff7e);
    pub const Hangul_switch: Keysym = Keysym(0x0000ff7e);
    pub const SunAltGraph: Keysym = Keysym(0x0000ff7e);
    pub const Num_Lock: Keysym = Keysym(0x0000ff7f);
    pub const KP_Space: Keysym = Keysym(0x0000ff80);
    pub const KP_Tab: Keysym = Keysym(0x0000ff89);
    pub const KP_Enter: Keysym = Keysym(0x0000ff8d);
    pub const KP_F1: Keysym = Keysym(0x0000ff91);
    pub const KP_F2: Keysym = Keysym(0x0000ff92);
    pub const KP_F3: Keysym = Keysym(0x0000ff93);
    pub const KP_F4: Keysym = Keysym(0x0000ff94);
    pub const KP_Home: Keysym = Keysym(0x0000ff95);
    pub const KP_Left: Keysym = Keysym(0x0000ff96);
    pub const KP_Up: Keysym = Keysym(0x0000ff97);
    pub const KP_Right: Keysym = Keysym(0x0000ff98);
    pub const KP_Down: Keysym = Keysym(0x0000ff99);
    pub const KP_Prior: Keysym = Keysym(0x0000ff9a);
    pub const KP_Page_Up: Keysym = Keysym(0x0000ff9a);
    pub const KP_Next: Keysym = Keysym(0x0000ff9b);
    pub const KP_Page_Down: Keysym = Keysym(0x0000ff9b);
    pub const KP_End: Keysym = Keysym(0x0000ff9c);
    pub const KP_Begin: Keysym = Keysym(0x0000ff9d);
    pub const KP_Insert: Keysym = Keysym(0x0000ff9e);
    pub const KP_Delete: Keysym = Keysym(0x0000ff9f);
    pub const KP_Multiply: Keysym = Keysym(0x0000ffaa);
    pub const KP_Add: Keysym = Keysym(0x0000ffab);
    pub const KP_Separator: Keysym = Keysym(0x0000ffac);
    pub const KP_Subtract: Keysym = Keysym(0x0000ffad);
    pub const KP_Decimal: Keysym = Keysym(0x0000ffae);
    pub const KP_Divide: Keysym = Keysym(0x0000ffaf);
    pub const KP_0: Keysym = Keysym(0x0000ffb0);
    pub const KP_1: Keysym = Keysym(0x0000ffb1);
    pub const KP_2: Keysym = Keysym(0x0000ffb2);
    pub const KP_3: Keysym = Keysym(0x0000ffb3);
    pub const KP_4: Keysym = Keysym(0x0000ffb4);
    pub const KP_5: Keysym = Keysym(0x0000ffb5);
    pub const KP_6: Keysym = Keysym(0x0000ffb6);
    pub const KP_7: Keysym = Keysym(0x0000ffb7);
    pub const KP_8: Keysym = Keysym(0x0000ffb8);
    pub const KP_9: Keysym = Keysym(0x0000ffb9);
    pub const KP_Equal: Keysym = Keysym(0x0000ffbd);
    pub const F1: Keysym = Keysym(0x0000ffbe);
    pub const F2: Keysym = Keysym(0x0000ffbf);
    pub const F3: Keysym = Keysym(0x0000ffc0);
    pub const F4: Keysym = Keysym(0x0000ffc1);
    pub const F5: Keysym = Keysym(0x0000ffc2);
    pub const F6: Keysym = Keysym(0x0000ffc3);
    pub const F7: Keysym = Keysym(0x0000ffc4);
    pub const F8: Keysym = Keysym(0x0000ffc5);
    pub const F9: Keysym = Keysym(0x0000ffc6);
    pub const F10: Keysym = Keysym(0x0000ffc7);
    pub const F11: Keysym = Keysym(0x0000ffc8);
    pub const L1: Keysym = Keysym(0x0000ffc8);
    pub const F12: Keysym = Keysym(0x0000ffc9);
    pub const L2: Keysym = Keysym(0x0000ffc9);
    pub const F13: Keysym = Keysym(0x0000ffca);
    pub const L3: Keysym = Keysym(0x0000ffca);
    pub const F14: Keysym = Keysym(0x0000ffcb);
    pub const L4: Keysym = Keysym(0x0000ffcb);
    pub const F15: Keysym = Keysym(0x0000ffcc);
    pub const L5: Keysym = Keysym(0x0000ffcc);
    pub const F16: Keysym = Keysym(0x0000ffcd);
    pub const L6: Keysym = Keysym(0x0000ffcd);
    pub const F17: Keysym = Keysym(0x0000ffce);
    pub const L7: Keysym = Keysym(0x0000ffce);
    pub const F18: Keysym = Keysym(0x0000ffcf);
    pub const L8: Keysym = Keysym(0x0000ffcf);
    pub const F19: Keysym = Keysym(0x0000ffd0);
    pub const L9: Keysym = Keysym(0x0000ffd0);
    pub const F20: Keysym = Keysym(0x0000ffd1);
    pub const L10: Keysym = Keysym(0x0000ffd1);
    pub const F21: Keysym = Keysym(0x0000ffd2);
    pub const R1: Keysym = Keysym(0x0000ffd2);
    pub const F22: Keysym = Keysym(0x0000ffd3);
    pub const R2: Keysym = Keysym(0x0000ffd3);
    pub const F23: Keysym = Keysym(0x0000ffd4);
    pub const R3: Keysym = Keysym(0x0000ffd4);
    pub const F24: Keysym = Keysym(0x0000ffd5);
    pub const R4: Keysym = Keysym(0x0000ffd5);
    pub const F25: Keysym = Keysym(0x0000ffd6);
    pub const R5: Keysym = Keysym(0x0000ffd6);
    pub const F26: Keysym = Keysym(0x0000ffd7);
    pub const R6: Keysym = Keysym(0x0000ffd7);
    pub const F27: Keysym = Keysym(0x0000ffd8);
    pub const R7: Keysym = Keysym(0x0000ffd8);
    pub const F28: Keysym = Keysym(0x0000ffd9);
    pub const R8: Keysym = Keysym(0x0000ffd9);
    pub const F29: Keysym = Keysym(0x0000ffda);
    pub const R9: Keysym = Keysym(0x0000ffda);
    pub const F30: Keysym = Keysym(0x0000ffdb);
    pub const R10: Keysym = Keysym(0x0000ffdb);
    pub const F31: Keysym = Keysym(0x0000ffdc);
    pub const R11: Keysym = Keysym(0x0000ffdc);
    pub const F32: Keysym = Keysym(0x0000ffdd);
    pub const R12: Keysym = Keysym(0x0000ffdd);
    pub const F33: Keysym = Keysym(0x0000ffde);
    pub const R13: Keysym = Keysym(0x0000ffde);
    pub const F34: Keysym = Keysym(0x0000ffdf);
    pub const R14: Keysym = Keysym(0x0000ffdf);
    pub const F35: Keysym = Keysym(0x0000ffe0);
    pub const R15: Keysym = Keysym(0x0000ffe0);
    pub const Shift_L: Keysym = Keysym(0x0000ffe1);
    pub const Shift_R: Keysym = Keysym(0x0000ffe2);
    pub const Control_L: Keysym = Keysym(0x0000ffe3);
    pub const Control_R: Keysym = Keysym(0x0000ffe4);
    pub const Caps_Lock: Keysym = Keysym(0x0000ffe5);
    pub const Shift_Lock: Keysym = Keysym(0x0000ffe6);
    pub const Meta_L: Keysym = Keysym(0x0000ffe7);
    pub const Meta_R: Keysym = Keysym(0x0000ffe8);
    pub const Alt_L: Keysym = Keysym(0x0000ffe9);
    pub const Alt_R: Keysym = Keysym(0x0000ffea);
    pub const Super_L: Keysym = Keysym(0x0000ffeb);
    pub const Super_R: Keysym = Keysym(0x0000ffec);
    pub const Hyper_L: Keysym = Keysym(0x0000ffed);
    pub const Hyper_R: Keysym = Keysym(0x0000ffee);
    pub const braille_dot_1: Keysym = Keysym(0x0000fff1);
    pub const braille_dot_2: Keysym = Keysym(0x0000fff2);
    pub const braille_dot_3: Keysym = Keysym(0x0000fff3);
    pub const braille_dot_4: Keysym = Keysym(0x0000fff4);
    pub const braille_dot_5: Keysym = Keysym(0x0000fff5);
    pub const braille_dot_6: Keysym = Keysym(0x0000fff6);
    pub const braille_dot_7: Keysym = Keysym(0x0000fff7);
    pub const braille_dot_8: Keysym = Keysym(0x0000fff8);
    pub const braille_dot_9: Keysym = Keysym(0x0000fff9);
    pub const braille_dot_10: Keysym = Keysym(0x0000fffa);
    pub const Delete: Keysym = Keysym(0x0000ffff);
    pub const VoidSymbol: Keysym = Keysym(0x00ffffff);
    pub const Ibreve: Keysym = Keysym(0x0100012c);
    pub const ibreve: Keysym = Keysym(0x0100012d);
    pub const Wcircumflex: Keysym = Keysym(0x01000174);
    pub const wcircumflex: Keysym = Keysym(0x01000175);
    pub const Ycircumflex: Keysym = Keysym(0x01000176);
    pub const ycircumflex: Keysym = Keysym(0x01000177);
    pub const SCHWA: Keysym = Keysym(0x0100018f);
    pub const Obarred: Keysym = Keysym(0x0100019f);
    pub const Ohorn: Keysym = Keysym(0x010001a0);
    pub const ohorn: Keysym = Keysym(0x010001a1);
    pub const Uhorn: Keysym = Keysym(0x010001af);
    pub const uhorn: Keysym = Keysym(0x010001b0);
    pub const Zstroke: Keysym = Keysym(0x010001b5);
    pub const zstroke: Keysym = Keysym(0x010001b6);
    pub const EZH: Keysym = Keysym(0x010001b7);
    pub const Ocaron: Keysym = Keysym(0x010001d1);
    pub const ocaron: Keysym = Keysym(0x010001d2);
    pub const Gcaron: Keysym = Keysym(0x010001e6);
    pub const gcaron: Keysym = Keysym(0x010001e7);
    pub const schwa: Keysym = Keysym(0x01000259);
    pub const obarred: Keysym = Keysym(0x01000275);
    pub const ezh: Keysym = Keysym(0x01000292);
    pub const combining_grave: Keysym = Keysym(0x01000300);
    pub const combining_acute: Keysym = Keysym(0x01000301);
    pub const combining_tilde: Keysym = Keysym(0x01000303);
    pub const combining_hook: Keysym = Keysym(0x01000309);
    pub const combining_belowdot: Keysym = Keysym(0x01000323);
    pub const Cyrillic_GHE_bar: Keysym = Keysym(0x01000492);
    pub const Cyrillic_ghe_bar: Keysym = Keysym(0x01000493);
    pub const Cyrillic_ZHE_descender: Keysym = Keysym(0x01000496);
    pub const Cyrillic_zhe_descender: Keysym = Keysym(0x01000497);
    pub const Cyrillic_KA_descender: Keysym = Keysym(0x0100049a);
    pub const Cyrillic_ka_descender: Keysym = Keysym(0x0100049b);
    pub const Cyrillic_KA_vertstroke: Keysym = Keysym(0x0100049c);
    pub const Cyrillic_ka_vertstroke: Keysym = Keysym(0x0100049d);
    pub const Cyrillic_EN_descender: Keysym = Keysym(0x010004a2);
    pub const Cyrillic_en_descender: Keysym = Keysym(0x010004a3);
    pub const Cyrillic_U_straight: Keysym = Keysym(0x010004ae);
    pub const Cyrillic_u_straight: Keysym = Keysym(0x010004af);
    pub const Cyrillic_U_straight_bar: Keysym = Keysym(0x010004b0);
    pub const Cyrillic_u_straight_bar: Keysym = Keysym(0x010004b1);
    pub const Cyrillic_HA_descender: Keysym = Keysym(0x010004b2);
    pub const Cyrillic_ha_descender: Keysym = Keysym(0x010004b3);
    pub const Cyrillic_CHE_descender: Keysym = Keysym(0x010004b6);
    pub const Cyrillic_che_descender: Keysym = Keysym(0x010004b7);
    pub const Cyrillic_CHE_vertstroke: Keysym = Keysym(0x010004b8);
    pub const Cyrillic_che_vertstroke: Keysym = Keysym(0x010004b9);
    pub const Cyrillic_SHHA: Keysym = Keysym(0x010004ba);
    pub const Cyrillic_shha: Keysym = Keysym(0x010004bb);
    pub const Cyrillic_SCHWA: Keysym = Keysym(0x010004d8);
    pub const Cyrillic_schwa: Keysym = Keysym(0x010004d9);
    pub const Cyrillic_I_macron: Keysym = Keysym(0x010004e2);
    pub const Cyrillic_i_macron: Keysym = Keysym(0x010004e3);
    pub const Cyrillic_O_bar: Keysym = Keysym(0x010004e8);
    pub const Cyrillic_o_bar: Keysym = Keysym(0x010004e9);
    pub const Cyrillic_U_macron: Keysym = Keysym(0x010004ee);
    pub const Cyrillic_u_macron: Keysym = Keysym(0x010004ef);
    pub const Armenian_AYB: Keysym = Keysym(0x01000531);
    pub const Armenian_BEN: Keysym = Keysym(0x01000532);
    pub const Armenian_GIM: Keysym = Keysym(0x01000533);
    pub const Armenian_DA: Keysym = Keysym(0x01000534);
    pub const Armenian_YECH: Keysym = Keysym(0x01000535);
    pub const Armenian_ZA: Keysym = Keysym(0x01000536);
    pub const Armenian_E: Keysym = Keysym(0x01000537);
    pub const Armenian_AT: Keysym = Keysym(0x01000538);
    pub const Armenian_TO: Keysym = Keysym(0x01000539);
    pub const Armenian_ZHE: Keysym = Keysym(0x0100053a);
    pub const Armenian_INI: Keysym = Keysym(0x0100053b);
    pub const Armenian_LYUN: Keysym = Keysym(0x0100053c);
    pub const Armenian_KHE: Keysym = Keysym(0x0100053d);
    pub const Armenian_TSA: Keysym = Keysym(0x0100053e);
    pub const Armenian_KEN: Keysym = Keysym(0x0100053f);
    pub const Armenian_HO: Keysym = Keysym(0x01000540);
    pub const Armenian_DZA: Keysym = Keysym(0x01000541);
    pub const Armenian_GHAT: Keysym = Keysym(0x01000542);
    pub const Armenian_TCHE: Keysym = Keysym(0x01000543);
    pub const Armenian_MEN: Keysym = Keysym(0x01000544);
    pub const Armenian_HI: Keysym = Keysym(0x01000545);
    pub const Armenian_NU: Keysym = Keysym(0x01000546);
    pub const Armenian_SHA: Keysym = Keysym(0x01000547);
    pub const Armenian_VO: Keysym = Keysym(0x01000548);
    pub const Armenian_CHA: Keysym = Keysym(0x01000549);
    pub const Armenian_PE: Keysym = Keysym(0x0100054a);
    pub const Armenian_JE: Keysym = Keysym(0x0100054b);
    pub const Armenian_RA: Keysym = Keysym(0x0100054c);
    pub const Armenian_SE: Keysym = Keysym(0x0100054d);
    pub const Armenian_VEV: Keysym = Keysym(0x0100054e);
    pub const Armenian_TYUN: Keysym = Keysym(0x0100054f);
    pub const Armenian_RE: Keysym = Keysym(0x01000550);
    pub const Armenian_TSO: Keysym = Keysym(0x01000551);
    pub const Armenian_VYUN: Keysym = Keysym(0x01000552);
    pub const Armenian_PYUR: Keysym = Keysym(0x01000553);
    pub const Armenian_KE: Keysym = Keysym(0x01000554);
    pub const Armenian_O: Keysym = Keysym(0x01000555);
    pub const Armenian_FE: Keysym = Keysym(0x01000556);
    pub const Armenian_apostrophe: Keysym = Keysym(0x0100055a);
    pub const Armenian_accent: Keysym = Keysym(0x0100055b);
    pub const Armenian_shesht: Keysym = Keysym(0x0100055b);
    pub const Armenian_exclam: Keysym = Keysym(0x0100055c);
    pub const Armenian_amanak: Keysym = Keysym(0x0100055c);
    pub const Armenian_separation_mark: Keysym = Keysym(0x0100055d);
    pub const Armenian_but: Keysym = Keysym(0x0100055d);
    pub const Armenian_question: Keysym = Keysym(0x0100055e);
    pub const Armenian_paruyk: Keysym = Keysym(0x0100055e);
    pub const Armenian_ayb: Keysym = Keysym(0x01000561);
    pub const Armenian_ben: Keysym = Keysym(0x01000562);
    pub const Armenian_gim: Keysym = Keysym(0x01000563);
    pub const Armenian_da: Keysym = Keysym(0x01000564);
    pub const Armenian_yech: Keysym = Keysym(0x01000565);
    pub const Armenian_za: Keysym = Keysym(0x01000566);
    pub const Armenian_e: Keysym = Keysym(0x01000567);
    pub const Armenian_at: Keysym = Keysym(0x01000568);
    pub const Armenian_to: Keysym = Keysym(0x01000569);
    pub const Armenian_zhe: Keysym = Keysym(0x0100056a);
    pub const Armenian_ini: Keysym = Keysym(0x0100056b);
    pub const Armenian_lyun: Keysym = Keysym(0x0100056c);
    pub const Armenian_khe: Keysym = Keysym(0x0100056d);
    pub const Armenian_tsa: Keysym = Keysym(0x0100056e);
    pub const Armenian_ken: Keysym = Keysym(0x0100056f);
    pub const Armenian_ho: Keysym = Keysym(0x01000570);
    pub const Armenian_dza: Keysym = Keysym(0x01000571);
    pub const Armenian_ghat: Keysym = Keysym(0x01000572);
    pub const Armenian_tche: Keysym = Keysym(0x01000573);
    pub const Armenian_men: Keysym = Keysym(0x01000574);
    pub const Armenian_hi: Keysym = Keysym(0x01000575);
    pub const Armenian_nu: Keysym = Keysym(0x01000576);
    pub const Armenian_sha: Keysym = Keysym(0x01000577);
    pub const Armenian_vo: Keysym = Keysym(0x01000578);
    pub const Armenian_cha: Keysym = Keysym(0x01000579);
    pub const Armenian_pe: Keysym = Keysym(0x0100057a);
    pub const Armenian_je: Keysym = Keysym(0x0100057b);
    pub const Armenian_ra: Keysym = Keysym(0x0100057c);
    pub const Armenian_se: Keysym = Keysym(0x0100057d);
    pub const Armenian_vev: Keysym = Keysym(0x0100057e);
    pub const Armenian_tyun: Keysym = Keysym(0x0100057f);
    pub const Armenian_re: Keysym = Keysym(0x01000580);
    pub const Armenian_tso: Keysym = Keysym(0x01000581);
    pub const Armenian_vyun: Keysym = Keysym(0x01000582);
    pub const Armenian_pyur: Keysym = Keysym(0x01000583);
    pub const Armenian_ke: Keysym = Keysym(0x01000584);
    pub const Armenian_o: Keysym = Keysym(0x01000585);
    pub const Armenian_fe: Keysym = Keysym(0x01000586);
    pub const Armenian_ligature_ew: Keysym = Keysym(0x01000587);
    pub const Armenian_full_stop: Keysym = Keysym(0x01000589);
    pub const Armenian_verjaket: Keysym = Keysym(0x01000589);
    pub const Armenian_hyphen: Keysym = Keysym(0x0100058a);
    pub const Armenian_yentamna: Keysym = Keysym(0x0100058a);
    pub const Arabic_madda_above: Keysym = Keysym(0x01000653);
    pub const Arabic_hamza_above: Keysym = Keysym(0x01000654);
    pub const Arabic_hamza_below: Keysym = Keysym(0x01000655);
    pub const Arabic_0: Keysym = Keysym(0x01000660);
    pub const Arabic_1: Keysym = Keysym(0x01000661);
    pub const Arabic_2: Keysym = Keysym(0x01000662);
    pub const Arabic_3: Keysym = Keysym(0x01000663);
    pub const Arabic_4: Keysym = Keysym(0x01000664);
    pub const Arabic_5: Keysym = Keysym(0x01000665);
    pub const Arabic_6: Keysym = Keysym(0x01000666);
    pub const Arabic_7: Keysym = Keysym(0x01000667);
    pub const Arabic_8: Keysym = Keysym(0x01000668);
    pub const Arabic_9: Keysym = Keysym(0x01000669);
    pub const Arabic_percent: Keysym = Keysym(0x0100066a);
    pub const Arabic_superscript_alef: Keysym = Keysym(0x01000670);
    pub const Arabic_tteh: Keysym = Keysym(0x01000679);
    pub const Arabic_peh: Keysym = Keysym(0x0100067e);
    pub const Arabic_tcheh: Keysym = Keysym(0x01000686);
    pub const Arabic_ddal: Keysym = Keysym(0x01000688);
    pub const Arabic_rreh: Keysym = Keysym(0x01000691);
    pub const Arabic_jeh: Keysym = Keysym(0x01000698);
    pub const Arabic_veh: Keysym = Keysym(0x010006a4);
    pub const Arabic_keheh: Keysym = Keysym(0x010006a9);
    pub const Arabic_gaf: Keysym = Keysym(0x010006af);
    pub const Arabic_noon_ghunna: Keysym = Keysym(0x010006ba);
    pub const Arabic_heh_doachashmee: Keysym = Keysym(0x010006be);
    pub const Arabic_heh_goal: Keysym = Keysym(0x010006c1);
    pub const Farsi_yeh: Keysym = Keysym(0x010006cc);
    pub const Arabic_farsi_yeh: Keysym = Keysym(0x010006cc);
    pub const Arabic_yeh_baree: Keysym = Keysym(0x010006d2);
    pub const Arabic_fullstop: Keysym = Keysym(0x010006d4);
    pub const Farsi_0: Keysym = Keysym(0x010006f0);
    pub const Farsi_1: Keysym = Keysym(0x010006f1);
    pub const Farsi_2: Keysym = Keysym(0x010006f2);
    pub const Farsi_3: Keysym = Keysym(0x010006f3);
    pub const Farsi_4: Keysym = Keysym(0x010006f4);
    pub const Farsi_5: Keysym = Keysym(0x010006f5);
    pub const Farsi_6: Keysym = Keysym(0x010006f6);
    pub const Farsi_7: Keysym = Keysym(0x010006f7);
    pub const Farsi_8: Keysym = Keysym(0x010006f8);
    pub const Farsi_9: Keysym = Keysym(0x010006f9);
    pub const Sinh_ng: Keysym = Keysym(0x01000d82);
    pub const Sinh_h2: Keysym = Keysym(0x01000d83);
    pub const Sinh_a: Keysym = Keysym(0x01000d85);
    pub const Sinh_aa: Keysym = Keysym(0x01000d86);
    pub const Sinh_ae: Keysym = Keysym(0x01000d87);
    pub const Sinh_aee: Keysym = Keysym(0x01000d88);
    pub const Sinh_i: Keysym = Keysym(0x01000d89);
    pub const Sinh_ii: Keysym = Keysym(0x01000d8a);
    pub const Sinh_u: Keysym = Keysym(0x01000d8b);
    pub const Sinh_uu: Keysym = Keysym(0x01000d8c);
    pub const Sinh_ri: Keysym = Keysym(0x01000d8d);
    pub const Sinh_rii: Keysym = Keysym(0x01000d8e);
    pub const Sinh_lu: Keysym = Keysym(0x01000d8f);
    pub const Sinh_luu: Keysym = Keysym(0x01000d90);
    pub const Sinh_e: Keysym = Keysym(0x01000d91);
    pub const Sinh_ee: Keysym = Keysym(0x01000d92);
    pub const Sinh_ai: Keysym = Keysym(0x01000d93);
    pub const Sinh_o: Keysym = Keysym(0x01000d94);
    pub const Sinh_oo: Keysym = Keysym(0x01000d95);
    pub const Sinh_au: Keysym = Keysym(0x01000d96);
    pub const Sinh_ka: Keysym = Keysym(0x01000d9a);
    pub const Sinh_kha: Keysym = Keysym(0x01000d9b);
    pub const Sinh_ga: Keysym = Keysym(0x01000d9c);
    pub const Sinh_gha: Keysym = Keysym(0x01000d9d);
    pub const Sinh_ng2: Keysym = Keysym(0x01000d9e);
    pub const Sinh_nga: Keysym = Keysym(0x01000d9f);
    pub const Sinh_ca: Keysym = Keysym(0x01000da0);
    pub const Sinh_cha: Keysym = Keysym(0x01000da1);
    pub const Sinh_ja: Keysym = Keysym(0x01000da2);
    pub const Sinh_jha: Keysym = Keysym(0x01000da3);
    pub const Sinh_nya: Keysym = Keysym(0x01000da4);
    pub const Sinh_jnya: Keysym = Keysym(0x01000da5);
    pub const Sinh_nja: Keysym = Keysym(0x01000da6);
    pub const Sinh_tta: Keysym = Keysym(0x01000da7);
    pub const Sinh_ttha: Keysym = Keysym(0x01000da8);
    pub const Sinh_dda: Keysym = Keysym(0x01000da9);
    pub const Sinh_ddha: Keysym = Keysym(0x01000daa);
    pub const Sinh_nna: Keysym = Keysym(0x01000dab);
    pub const Sinh_ndda: Keysym = Keysym(0x01000dac);
    pub const Sinh_tha: Keysym = Keysym(0x01000dad);
    pub const Sinh_thha: Keysym = Keysym(0x01000dae);
    pub const Sinh_dha: Keysym = Keysym(0x01000daf);
    pub const Sinh_dhha: Keysym = Keysym(0x01000db0);
    pub const Sinh_na: Keysym = Keysym(0x01000db1);
    pub const Sinh_ndha: Keysym = Keysym(0x01000db3);
    pub const Sinh_pa: Keysym = Keysym(0x01000db4);
    pub const Sinh_pha: Keysym = Keysym(0x01000db5);
    pub const Sinh_ba: Keysym = Keysym(0x01000db6);
    pub const Sinh_bha: Keysym = Keysym(0x01000db7);
    pub const Sinh_ma: Keysym = Keysym(0x01000db8);
    pub const Sinh_mba: Keysym = Keysym(0x01000db9);
    pub const Sinh_ya: Keysym = Keysym(0x01000dba);
    pub const Sinh_ra: Keysym = Keysym(0x01000dbb);
    pub const Sinh_la: Keysym = Keysym(0x01000dbd);
    pub const Sinh_va: Keysym = Keysym(0x01000dc0);
    pub const Sinh_sha: Keysym = Keysym(0x01000dc1);
    pub const Sinh_ssha: Keysym = Keysym(0x01000dc2);
    pub const Sinh_sa: Keysym = Keysym(0x01000dc3);
    pub const Sinh_ha: Keysym = Keysym(0x01000dc4);
    pub const Sinh_lla: Keysym = Keysym(0x01000dc5);
    pub const Sinh_fa: Keysym = Keysym(0x01000dc6);
    pub const Sinh_al: Keysym = Keysym(0x01000dca);
    pub const Sinh_aa2: Keysym = Keysym(0x01000dcf);
    pub const Sinh_ae2: Keysym = Keysym(0x01000dd0);
    pub const Sinh_aee2: Keysym = Keysym(0x01000dd1);
    pub const Sinh_i2: Keysym = Keysym(0x01000dd2);
    pub const Sinh_ii2: Keysym = Keysym(0x01000dd3);
    pub const Sinh_u2: Keysym = Keysym(0x01000dd4);
    pub const Sinh_uu2: Keysym = Keysym(0x01000dd6);
    pub const Sinh_ru2: Keysym = Keysym(0x01000dd8);
    pub const Sinh_e2: Keysym = Keysym(0x01000dd9);
    pub const Sinh_ee2: Keysym = Keysym(0x01000dda);
    pub const Sinh_ai2: Keysym = Keysym(0x01000ddb);
    pub const Sinh_o2: Keysym = Keysym(0x01000ddc);
    pub const Sinh_oo2: Keysym = Keysym(0x01000ddd);
    pub const Sinh_au2: Keysym = Keysym(0x01000dde);
    pub const Sinh_lu2: Keysym = Keysym(0x01000ddf);
    pub const Sinh_ruu2: Keysym = Keysym(0x01000df2);
    pub const Sinh_luu2: Keysym = Keysym(0x01000df3);
    pub const Sinh_kunddaliya: Keysym = Keysym(0x01000df4);
    pub const Georgian_an: Keysym = Keysym(0x010010d0);
    pub const Georgian_ban: Keysym = Keysym(0x010010d1);
    pub const Georgian_gan: Keysym = Keysym(0x010010d2);
    pub const Georgian_don: Keysym = Keysym(0x010010d3);
    pub const Georgian_en: Keysym = Keysym(0x010010d4);
    pub const Georgian_vin: Keysym = Keysym(0x010010d5);
    pub const Georgian_zen: Keysym = Keysym(0x010010d6);
    pub const Georgian_tan: Keysym = Keysym(0x010010d7);
    pub const Georgian_in: Keysym = Keysym(0x010010d8);
    pub const Georgian_kan: Keysym = Keysym(0x010010d9);
    pub const Georgian_las: Keysym = Keysym(0x010010da);
    pub const Georgian_man: Keysym = Keysym(0x010010db);
    pub const Georgian_nar: Keysym = Keysym(0x010010dc);
    pub const Georgian_on: Keysym = Keysym(0x010010dd);
    pub const Georgian_par: Keysym = Keysym(0x010010de);
    pub const Georgian_zhar: Keysym = Keysym(0x010010df);
    pub const Georgian_rae: Keysym = Keysym(0x010010e0);
    pub const Georgian_san: Keysym = Keysym(0x010010e1);
    pub const Georgian_tar: Keysym = Keysym(0x010010e2);
    pub const Georgian_un: Keysym = Keysym(0x010010e3);
    pub const Georgian_phar: Keysym = Keysym(0x010010e4);
    pub const Georgian_khar: Keysym = Keysym(0x010010e5);
    pub const Georgian_ghan: Keysym = Keysym(0x010010e6);
    pub const Georgian_qar: Keysym = Keysym(0x010010e7);
    pub const Georgian_shin: Keysym = Keysym(0x010010e8);
    pub const Georgian_chin: Keysym = Keysym(0x010010e9);
    pub const Georgian_can: Keysym = Keysym(0x010010ea);
    pub const Georgian_jil: Keysym = Keysym(0x010010eb);
    pub const Georgian_cil: Keysym = Keysym(0x010010ec);
    pub const Georgian_char: Keysym = Keysym(0x010010ed);
    pub const Georgian_xan: Keysym = Keysym(0x010010ee);
    pub const Georgian_jhan: Keysym = Keysym(0x010010ef);
    pub const Georgian_hae: Keysym = Keysym(0x010010f0);
    pub const Georgian_he: Keysym = Keysym(0x010010f1);
    pub const Georgian_hie: Keysym = Keysym(0x010010f2);
    pub const Georgian_we: Keysym = Keysym(0x010010f3);
    pub const Georgian_har: Keysym = Keysym(0x010010f4);
    pub const Georgian_hoe: Keysym = Keysym(0x010010f5);
    pub const Georgian_fi: Keysym = Keysym(0x010010f6);
    pub const Babovedot: Keysym = Keysym(0x01001e02);
    pub const babovedot: Keysym = Keysym(0x01001e03);
    pub const Dabovedot: Keysym = Keysym(0x01001e0a);
    pub const dabovedot: Keysym = Keysym(0x01001e0b);
    pub const Fabovedot: Keysym = Keysym(0x01001e1e);
    pub const fabovedot: Keysym = Keysym(0x01001e1f);
    pub const Lbelowdot: Keysym = Keysym(0x01001e36);
    pub const lbelowdot: Keysym = Keysym(0x01001e37);
    pub const Mabovedot: Keysym = Keysym(0x01001e40);
    pub const mabovedot: Keysym = Keysym(0x01001e41);
    pub const Pabovedot: Keysym = Keysym(0x01001e56);
    pub const pabovedot: Keysym = Keysym(0x01001e57);
    pub const Sabovedot: Keysym = Keysym(0x01001e60);
    pub const sabovedot: Keysym = Keysym(0x01001e61);
    pub const Tabovedot: Keysym = Keysym(0x01001e6a);
    pub const tabovedot: Keysym = Keysym(0x01001e6b);
    pub const Wgrave: Keysym = Keysym(0x01001e80);
    pub const wgrave: Keysym = Keysym(0x01001e81);
    pub const Wacute: Keysym = Keysym(0x01001e82);
    pub const wacute: Keysym = Keysym(0x01001e83);
    pub const Wdiaeresis: Keysym = Keysym(0x01001e84);
    pub const wdiaeresis: Keysym = Keysym(0x01001e85);
    pub const Xabovedot: Keysym = Keysym(0x01001e8a);
    pub const xabovedot: Keysym = Keysym(0x01001e8b);
    pub const SSHARP: Keysym = Keysym(0x01001e9e);
    pub const Abelowdot: Keysym = Keysym(0x01001ea0);
    pub const abelowdot: Keysym = Keysym(0x01001ea1);
    pub const Ahook: Keysym = Keysym(0x01001ea2);
    pub const ahook: Keysym = Keysym(0x01001ea3);
    pub const Acircumflexacute: Keysym = Keysym(0x01001ea4);
    pub const acircumflexacute: Keysym = Keysym(0x01001ea5);
    pub const Acircumflexgrave: Keysym = Keysym(0x01001ea6);
    pub const acircumflexgrave: Keysym = Keysym(0x01001ea7);
    pub const Acircumflexhook: Keysym = Keysym(0x01001ea8);
    pub const acircumflexhook: Keysym = Keysym(0x01001ea9);
    pub const Acircumflextilde: Keysym = Keysym(0x01001eaa);
    pub const acircumflextilde: Keysym = Keysym(0x01001eab);
    pub const Acircumflexbelowdot: Keysym = Keysym(0x01001eac);
    pub const acircumflexbelowdot: Keysym = Keysym(0x01001ead);
    pub const Abreveacute: Keysym = Keysym(0x01001eae);
    pub const abreveacute: Keysym = Keysym(0x01001eaf);
    pub const Abrevegrave: Keysym = Keysym(0x01001eb0);
    pub const abrevegrave: Keysym = Keysym(0x01001eb1);
    pub const Abrevehook: Keysym = Keysym(0x01001eb2);
    pub const abrevehook: Keysym = Keysym(0x01001eb3);
    pub const Abrevetilde: Keysym = Keysym(0x01001eb4);
    pub const abrevetilde: Keysym = Keysym(0x01001eb5);
    pub const Abrevebelowdot: Keysym = Keysym(0x01001eb6);
    pub const abrevebelowdot: Keysym = Keysym(0x01001eb7);
    pub const Ebelowdot: Keysym = Keysym(0x01001eb8);
    pub const ebelowdot: Keysym = Keysym(0x01001eb9);
    pub const Ehook: Keysym = Keysym(0x01001eba);
    pub const ehook: Keysym = Keysym(0x01001ebb);
    pub const Etilde: Keysym = Keysym(0x01001ebc);
    pub const etilde: Keysym = Keysym(0x01001ebd);
    pub const Ecircumflexacute: Keysym = Keysym(0x01001ebe);
    pub const ecircumflexacute: Keysym = Keysym(0x01001ebf);
    pub const Ecircumflexgrave: Keysym = Keysym(0x01001ec0);
    pub const ecircumflexgrave: Keysym = Keysym(0x01001ec1);
    pub const Ecircumflexhook: Keysym = Keysym(0x01001ec2);
    pub const ecircumflexhook: Keysym = Keysym(0x01001ec3);
    pub const Ecircumflextilde: Keysym = Keysym(0x01001ec4);
    pub const ecircumflextilde: Keysym = Keysym(0x01001ec5);
    pub const Ecircumflexbelowdot: Keysym = Keysym(0x01001ec6);
    pub const ecircumflexbelowdot: Keysym = Keysym(0x01001ec7);
    pub const Ihook: Keysym = Keysym(0x01001ec8);
    pub const ihook: Keysym = Keysym(0x01001ec9);
    pub const Ibelowdot: Keysym = Keysym(0x01001eca);
    pub const ibelowdot: Keysym = Keysym(0x01001ecb);
    pub const Obelowdot: Keysym = Keysym(0x01001ecc);
    pub const obelowdot: Keysym = Keysym(0x01001ecd);
    pub const Ohook: Keysym = Keysym(0x01001ece);
    pub const ohook: Keysym = Keysym(0x01001ecf);
    pub const Ocircumflexacute: Keysym = Keysym(0x01001ed0);
    pub const ocircumflexacute: Keysym = Keysym(0x01001ed1);
    pub const Ocircumflexgrave: Keysym = Keysym(0x01001ed2);
    pub const ocircumflexgrave: Keysym = Keysym(0x01001ed3);
    pub const Ocircumflexhook: Keysym = Keysym(0x01001ed4);
    pub const ocircumflexhook: Keysym = Keysym(0x01001ed5);
    pub const Ocircumflextilde: Keysym = Keysym(0x01001ed6);
    pub const ocircumflextilde: Keysym = Keysym(0x01001ed7);
    pub const Ocircumflexbelowdot: Keysym = Keysym(0x01001ed8);
    pub const ocircumflexbelowdot: Keysym = Keysym(0x01001ed9);
    pub const Ohornacute: Keysym = Keysym(0x01001eda);
    pub const ohornacute: Keysym = Keysym(0x01001edb);
    pub const Ohorngrave: Keysym = Keysym(0x01001edc);
    pub const ohorngrave: Keysym = Keysym(0x01001edd);
    pub const Ohornhook: Keysym = Keysym(0x01001ede);
    pub const ohornhook: Keysym = Keysym(0x01001edf);
    pub const Ohorntilde: Keysym = Keysym(0x01001ee0);
    pub const ohorntilde: Keysym = Keysym(0x01001ee1);
    pub const Ohornbelowdot: Keysym = Keysym(0x01001ee2);
    pub const ohornbelowdot: Keysym = Keysym(0x01001ee3);
    pub const Ubelowdot: Keysym = Keysym(0x01001ee4);
    pub const ubelowdot: Keysym = Keysym(0x01001ee5);
    pub const Uhook: Keysym = Keysym(0x01001ee6);
    pub const uhook: Keysym = Keysym(0x01001ee7);
    pub const Uhornacute: Keysym = Keysym(0x01001ee8);
    pub const uhornacute: Keysym = Keysym(0x01001ee9);
    pub const Uhorngrave: Keysym = Keysym(0x01001eea);
    pub const uhorngrave: Keysym = Keysym(0x01001eeb);
    pub const Uhornhook: Keysym = Keysym(0x01001eec);
    pub const uhornhook: Keysym = Keysym(0x01001eed);
    pub const Uhorntilde: Keysym = Keysym(0x01001eee);
    pub const uhorntilde: Keysym = Keysym(0x01001eef);
    pub const Uhornbelowdot: Keysym = Keysym(0x01001ef0);
    pub const uhornbelowdot: Keysym = Keysym(0x01001ef1);
    pub const Ygrave: Keysym = Keysym(0x01001ef2);
    pub const ygrave: Keysym = Keysym(0x01001ef3);
    pub const Ybelowdot: Keysym = Keysym(0x01001ef4);
    pub const ybelowdot: Keysym = Keysym(0x01001ef5);
    pub const Yhook: Keysym = Keysym(0x01001ef6);
    pub const yhook: Keysym = Keysym(0x01001ef7);
    pub const Ytilde: Keysym = Keysym(0x01001ef8);
    pub const ytilde: Keysym = Keysym(0x01001ef9);
    pub const leftsingleanglequotemark: Keysym = Keysym(0x01002039);
    pub const rightsingleanglequotemark: Keysym = Keysym(0x0100203a);
    pub const zerosuperior: Keysym = Keysym(0x01002070);
    pub const foursuperior: Keysym = Keysym(0x01002074);
    pub const fivesuperior: Keysym = Keysym(0x01002075);
    pub const sixsuperior: Keysym = Keysym(0x01002076);
    pub const sevensuperior: Keysym = Keysym(0x01002077);
    pub const eightsuperior: Keysym = Keysym(0x01002078);
    pub const ninesuperior: Keysym = Keysym(0x01002079);
    pub const zerosubscript: Keysym = Keysym(0x01002080);
    pub const onesubscript: Keysym = Keysym(0x01002081);
    pub const twosubscript: Keysym = Keysym(0x01002082);
    pub const threesubscript: Keysym = Keysym(0x01002083);
    pub const foursubscript: Keysym = Keysym(0x01002084);
    pub const fivesubscript: Keysym = Keysym(0x01002085);
    pub const sixsubscript: Keysym = Keysym(0x01002086);
    pub const sevensubscript: Keysym = Keysym(0x01002087);
    pub const eightsubscript: Keysym = Keysym(0x01002088);
    pub const ninesubscript: Keysym = Keysym(0x01002089);
    pub const EcuSign: Keysym = Keysym(0x010020a0);
    pub const ColonSign: Keysym = Keysym(0x010020a1);
    pub const CruzeiroSign: Keysym = Keysym(0x010020a2);
    pub const FFrancSign: Keysym = Keysym(0x010020a3);
    pub const LiraSign: Keysym = Keysym(0x010020a4);
    pub const MillSign: Keysym = Keysym(0x010020a5);
    pub const NairaSign: Keysym = Keysym(0x010020a6);
    pub const PesetaSign: Keysym = Keysym(0x010020a7);
    pub const RupeeSign: Keysym = Keysym(0x010020a8);
    pub const WonSign: Keysym = Keysym(0x010020a9);
    pub const NewSheqelSign: Keysym = Keysym(0x010020aa);
    pub const DongSign: Keysym = Keysym(0x010020ab);
    pub const partdifferential: Keysym = Keysym(0x01002202);
    pub const emptyset: Keysym = Keysym(0x01002205);
    pub const elementof: Keysym = Keysym(0x01002208);
    pub const notelementof: Keysym = Keysym(0x01002209);
    pub const containsas: Keysym = Keysym(0x0100220b);
    pub const squareroot: Keysym = Keysym(0x0100221a);
    pub const cuberoot: Keysym = Keysym(0x0100221b);
    pub const fourthroot: Keysym = Keysym(0x0100221c);
    pub const dintegral: Keysym = Keysym(0x0100222c);
    pub const tintegral: Keysym = Keysym(0x0100222d);
    pub const because: Keysym = Keysym(0x01002235);
    pub const notapproxeq: Keysym = Keysym(0x01002247);
    pub const approxeq: Keysym = Keysym(0x01002248);
    pub const notidentical: Keysym = Keysym(0x01002262);
    pub const stricteq: Keysym = Keysym(0x01002263);
    pub const braille_blank: Keysym = Keysym(0x01002800);
    pub const braille_dots_1: Keysym = Keysym(0x01002801);
    pub const braille_dots_2: Keysym = Keysym(0x01002802);
    pub const braille_dots_12: Keysym = Keysym(0x01002803);
    pub const braille_dots_3: Keysym = Keysym(0x01002804);
    pub const braille_dots_13: Keysym = Keysym(0x01002805);
    pub const braille_dots_23: Keysym = Keysym(0x01002806);
    pub const braille_dots_123: Keysym = Keysym(0x01002807);
    pub const braille_dots_4: Keysym = Keysym(0x01002808);
    pub const braille_dots_14: Keysym = Keysym(0x01002809);
    pub const braille_dots_24: Keysym = Keysym(0x0100280a);
    pub const braille_dots_124: Keysym = Keysym(0x0100280b);
    pub const braille_dots_34: Keysym = Keysym(0x0100280c);
    pub const braille_dots_134: Keysym = Keysym(0x0100280d);
    pub const braille_dots_234: Keysym = Keysym(0x0100280e);
    pub const braille_dots_1234: Keysym = Keysym(0x0100280f);
    pub const braille_dots_5: Keysym = Keysym(0x01002810);
    pub const braille_dots_15: Keysym = Keysym(0x01002811);
    pub const braille_dots_25: Keysym = Keysym(0x01002812);
    pub const braille_dots_125: Keysym = Keysym(0x01002813);
    pub const braille_dots_35: Keysym = Keysym(0x01002814);
    pub const braille_dots_135: Keysym = Keysym(0x01002815);
    pub const braille_dots_235: Keysym = Keysym(0x01002816);
    pub const braille_dots_1235: Keysym = Keysym(0x01002817);
    pub const braille_dots_45: Keysym = Keysym(0x01002818);
    pub const braille_dots_145: Keysym = Keysym(0x01002819);
    pub const braille_dots_245: Keysym = Keysym(0x0100281a);
    pub const braille_dots_1245: Keysym = Keysym(0x0100281b);
    pub const braille_dots_345: Keysym = Keysym(0x0100281c);
    pub const braille_dots_1345: Keysym = Keysym(0x0100281d);
    pub const braille_dots_2345: Keysym = Keysym(0x0100281e);
    pub const braille_dots_12345: Keysym = Keysym(0x0100281f);
    pub const braille_dots_6: Keysym = Keysym(0x01002820);
    pub const braille_dots_16: Keysym = Keysym(0x01002821);
    pub const braille_dots_26: Keysym = Keysym(0x01002822);
    pub const braille_dots_126: Keysym = Keysym(0x01002823);
    pub const braille_dots_36: Keysym = Keysym(0x01002824);
    pub const braille_dots_136: Keysym = Keysym(0x01002825);
    pub const braille_dots_236: Keysym = Keysym(0x01002826);
    pub const braille_dots_1236: Keysym = Keysym(0x01002827);
    pub const braille_dots_46: Keysym = Keysym(0x01002828);
    pub const braille_dots_146: Keysym = Keysym(0x01002829);
    pub const braille_dots_246: Keysym = Keysym(0x0100282a);
    pub const braille_dots_1246: Keysym = Keysym(0x0100282b);
    pub const braille_dots_346: Keysym = Keysym(0x0100282c);
    pub const braille_dots_1346: Keysym = Keysym(0x0100282d);
    pub const braille_dots_2346: Keysym = Keysym(0x0100282e);
    pub const braille_dots_12346: Keysym = Keysym(0x0100282f);
    pub const braille_dots_56: Keysym = Keysym(0x01002830);
    pub const braille_dots_156: Keysym = Keysym(0x01002831);
    pub const braille_dots_256: Keysym = Keysym(0x01002832);
    pub const braille_dots_1256: Keysym = Keysym(0x01002833);
    pub const braille_dots_356: Keysym = Keysym(0x01002834);
    pub const braille_dots_1356: Keysym = Keysym(0x01002835);
    pub const braille_dots_2356: Keysym = Keysym(0x01002836);
    pub const braille_dots_12356: Keysym = Keysym(0x01002837);
    pub const braille_dots_456: Keysym = Keysym(0x01002838);
    pub const braille_dots_1456: Keysym = Keysym(0x01002839);
    pub const braille_dots_2456: Keysym = Keysym(0x0100283a);
    pub const braille_dots_12456: Keysym = Keysym(0x0100283b);
    pub const braille_dots_3456: Keysym = Keysym(0x0100283c);
    pub const braille_dots_13456: Keysym = Keysym(0x0100283d);
    pub const braille_dots_23456: Keysym = Keysym(0x0100283e);
    pub const braille_dots_123456: Keysym = Keysym(0x0100283f);
    pub const braille_dots_7: Keysym = Keysym(0x01002840);
    pub const braille_dots_17: Keysym = Keysym(0x01002841);
    pub const braille_dots_27: Keysym = Keysym(0x01002842);
    pub const braille_dots_127: Keysym = Keysym(0x01002843);
    pub const braille_dots_37: Keysym = Keysym(0x01002844);
    pub const braille_dots_137: Keysym = Keysym(0x01002845);
    pub const braille_dots_237: Keysym = Keysym(0x01002846);
    pub const braille_dots_1237: Keysym = Keysym(0x01002847);
    pub const braille_dots_47: Keysym = Keysym(0x01002848);
    pub const braille_dots_147: Keysym = Keysym(0x01002849);
    pub const braille_dots_247: Keysym = Keysym(0x0100284a);
    pub const braille_dots_1247: Keysym = Keysym(0x0100284b);
    pub const braille_dots_347: Keysym = Keysym(0x0100284c);
    pub const braille_dots_1347: Keysym = Keysym(0x0100284d);
    pub const braille_dots_2347: Keysym = Keysym(0x0100284e);
    pub const braille_dots_12347: Keysym = Keysym(0x0100284f);
    pub const braille_dots_57: Keysym = Keysym(0x01002850);
    pub const braille_dots_157: Keysym = Keysym(0x01002851);
    pub const braille_dots_257: Keysym = Keysym(0x01002852);
    pub const braille_dots_1257: Keysym = Keysym(0x01002853);
    pub const braille_dots_357: Keysym = Keysym(0x01002854);
    pub const braille_dots_1357: Keysym = Keysym(0x01002855);
    pub const braille_dots_2357: Keysym = Keysym(0x01002856);
    pub const braille_dots_12357: Keysym = Keysym(0x01002857);
    pub const braille_dots_457: Keysym = Keysym(0x01002858);
    pub const braille_dots_1457: Keysym = Keysym(0x01002859);
    pub const braille_dots_2457: Keysym = Keysym(0x0100285a);
    pub const braille_dots_12457: Keysym = Keysym(0x0100285b);
    pub const braille_dots_3457: Keysym = Keysym(0x0100285c);
    pub const braille_dots_13457: Keysym = Keysym(0x0100285d);
    pub const braille_dots_23457: Keysym = Keysym(0x0100285e);
    pub const braille_dots_123457: Keysym = Keysym(0x0100285f);
    pub const braille_dots_67: Keysym = Keysym(0x01002860);
    pub const braille_dots_167: Keysym = Keysym(0x01002861);
    pub const braille_dots_267: Keysym = Keysym(0x01002862);
    pub const braille_dots_1267: Keysym = Keysym(0x01002863);
    pub const braille_dots_367: Keysym = Keysym(0x01002864);
    pub const braille_dots_1367: Keysym = Keysym(0x01002865);
    pub const braille_dots_2367: Keysym = Keysym(0x01002866);
    pub const braille_dots_12367: Keysym = Keysym(0x01002867);
    pub const braille_dots_467: Keysym = Keysym(0x01002868);
    pub const braille_dots_1467: Keysym = Keysym(0x01002869);
    pub const braille_dots_2467: Keysym = Keysym(0x0100286a);
    pub const braille_dots_12467: Keysym = Keysym(0x0100286b);
    pub const braille_dots_3467: Keysym = Keysym(0x0100286c);
    pub const braille_dots_13467: Keysym = Keysym(0x0100286d);
    pub const braille_dots_23467: Keysym = Keysym(0x0100286e);
    pub const braille_dots_123467: Keysym = Keysym(0x0100286f);
    pub const braille_dots_567: Keysym = Keysym(0x01002870);
    pub const braille_dots_1567: Keysym = Keysym(0x01002871);
    pub const braille_dots_2567: Keysym = Keysym(0x01002872);
    pub const braille_dots_12567: Keysym = Keysym(0x01002873);
    pub const braille_dots_3567: Keysym = Keysym(0x01002874);
    pub const braille_dots_13567: Keysym = Keysym(0x01002875);
    pub const braille_dots_23567: Keysym = Keysym(0x01002876);
    pub const braille_dots_123567: Keysym = Keysym(0x01002877);
    pub const braille_dots_4567: Keysym = Keysym(0x01002878);
    pub const braille_dots_14567: Keysym = Keysym(0x01002879);
    pub const braille_dots_24567: Keysym = Keysym(0x0100287a);
    pub const braille_dots_124567: Keysym = Keysym(0x0100287b);
    pub const braille_dots_34567: Keysym = Keysym(0x0100287c);
    pub const braille_dots_134567: Keysym = Keysym(0x0100287d);
    pub const braille_dots_234567: Keysym = Keysym(0x0100287e);
    pub const braille_dots_1234567: Keysym = Keysym(0x0100287f);
    pub const braille_dots_8: Keysym = Keysym(0x01002880);
    pub const braille_dots_18: Keysym = Keysym(0x01002881);
    pub const braille_dots_28: Keysym = Keysym(0x01002882);
    pub const braille_dots_128: Keysym = Keysym(0x01002883);
    pub const braille_dots_38: Keysym = Keysym(0x01002884);
    pub const braille_dots_138: Keysym = Keysym(0x01002885);
    pub const braille_dots_238: Keysym = Keysym(0x01002886);
    pub const braille_dots_1238: Keysym = Keysym(0x01002887);
    pub const braille_dots_48: Keysym = Keysym(0x01002888);
    pub const braille_dots_148: Keysym = Keysym(0x01002889);
    pub const braille_dots_248: Keysym = Keysym(0x0100288a);
    pub const braille_dots_1248: Keysym = Keysym(0x0100288b);
    pub const braille_dots_348: Keysym = Keysym(0x0100288c);
    pub const braille_dots_1348: Keysym = Keysym(0x0100288d);
    pub const braille_dots_2348: Keysym = Keysym(0x0100288e);
    pub const braille_dots_12348: Keysym = Keysym(0x0100288f);
    pub const braille_dots_58: Keysym = Keysym(0x01002890);
    pub const braille_dots_158: Keysym = Keysym(0x01002891);
    pub const braille_dots_258: Keysym = Keysym(0x01002892);
    pub const braille_dots_1258: Keysym = Keysym(0x01002893);
    pub const braille_dots_358: Keysym = Keysym(0x01002894);
    pub const braille_dots_1358: Keysym = Keysym(0x01002895);
    pub const braille_dots_2358: Keysym = Keysym(0x01002896);
    pub const braille_dots_12358: Keysym = Keysym(0x01002897);
    pub const braille_dots_458: Keysym = Keysym(0x01002898);
    pub const braille_dots_1458: Keysym = Keysym(0x01002899);
    pub const braille_dots_2458: Keysym = Keysym(0x0100289a);
    pub const braille_dots_12458: Keysym = Keysym(0x0100289b);
    pub const braille_dots_3458: Keysym = Keysym(0x0100289c);
    pub const braille_dots_13458: Keysym = Keysym(0x0100289d);
    pub const braille_dots_23458: Keysym = Keysym(0x0100289e);
    pub const braille_dots_123458: Keysym = Keysym(0x0100289f);
    pub const braille_dots_68: Keysym = Keysym(0x010028a0);
    pub const braille_dots_168: Keysym = Keysym(0x010028a1);
    pub const braille_dots_268: Keysym = Keysym(0x010028a2);
    pub const braille_dots_1268: Keysym = Keysym(0x010028a3);
    pub const braille_dots_368: Keysym = Keysym(0x010028a4);
    pub const braille_dots_1368: Keysym = Keysym(0x010028a5);
    pub const braille_dots_2368: Keysym = Keysym(0x010028a6);
    pub const braille_dots_12368: Keysym = Keysym(0x010028a7);
    pub const braille_dots_468: Keysym = Keysym(0x010028a8);
    pub const braille_dots_1468: Keysym = Keysym(0x010028a9);
    pub const braille_dots_2468: Keysym = Keysym(0x010028aa);
    pub const braille_dots_12468: Keysym = Keysym(0x010028ab);
    pub const braille_dots_3468: Keysym = Keysym(0x010028ac);
    pub const braille_dots_13468: Keysym = Keysym(0x010028ad);
    pub const braille_dots_23468: Keysym = Keysym(0x010028ae);
    pub const braille_dots_123468: Keysym = Keysym(0x010028af);
    pub const braille_dots_568: Keysym = Keysym(0x010028b0);
    pub const braille_dots_1568: Keysym = Keysym(0x010028b1);
    pub const braille_dots_2568: Keysym = Keysym(0x010028b2);
    pub const braille_dots_12568: Keysym = Keysym(0x010028b3);
    pub const braille_dots_3568: Keysym = Keysym(0x010028b4);
    pub const braille_dots_13568: Keysym = Keysym(0x010028b5);
    pub const braille_dots_23568: Keysym = Keysym(0x010028b6);
    pub const braille_dots_123568: Keysym = Keysym(0x010028b7);
    pub const braille_dots_4568: Keysym = Keysym(0x010028b8);
    pub const braille_dots_14568: Keysym = Keysym(0x010028b9);
    pub const braille_dots_24568: Keysym = Keysym(0x010028ba);
    pub const braille_dots_124568: Keysym = Keysym(0x010028bb);
    pub const braille_dots_34568: Keysym = Keysym(0x010028bc);
    pub const braille_dots_134568: Keysym = Keysym(0x010028bd);
    pub const braille_dots_234568: Keysym = Keysym(0x010028be);
    pub const braille_dots_1234568: Keysym = Keysym(0x010028bf);
    pub const braille_dots_78: Keysym = Keysym(0x010028c0);
    pub const braille_dots_178: Keysym = Keysym(0x010028c1);
    pub const braille_dots_278: Keysym = Keysym(0x010028c2);
    pub const braille_dots_1278: Keysym = Keysym(0x010028c3);
    pub const braille_dots_378: Keysym = Keysym(0x010028c4);
    pub const braille_dots_1378: Keysym = Keysym(0x010028c5);
    pub const braille_dots_2378: Keysym = Keysym(0x010028c6);
    pub const braille_dots_12378: Keysym = Keysym(0x010028c7);
    pub const braille_dots_478: Keysym = Keysym(0x010028c8);
    pub const braille_dots_1478: Keysym = Keysym(0x010028c9);
    pub const braille_dots_2478: Keysym = Keysym(0x010028ca);
    pub const braille_dots_12478: Keysym = Keysym(0x010028cb);
    pub const braille_dots_3478: Keysym = Keysym(0x010028cc);
    pub const braille_dots_13478: Keysym = Keysym(0x010028cd);
    pub const braille_dots_23478: Keysym = Keysym(0x010028ce);
    pub const braille_dots_123478: Keysym = Keysym(0x010028cf);
    pub const braille_dots_578: Keysym = Keysym(0x010028d0);
    pub const braille_dots_1578: Keysym = Keysym(0x010028d1);
    pub const braille_dots_2578: Keysym = Keysym(0x010028d2);
    pub const braille_dots_12578: Keysym = Keysym(0x010028d3);
    pub const braille_dots_3578: Keysym = Keysym(0x010028d4);
    pub const braille_dots_13578: Keysym = Keysym(0x010028d5);
    pub const braille_dots_23578: Keysym = Keysym(0x010028d6);
    pub const braille_dots_123578: Keysym = Keysym(0x010028d7);
    pub const braille_dots_4578: Keysym = Keysym(0x010028d8);
    pub const braille_dots_14578: Keysym = Keysym(0x010028d9);
    pub const braille_dots_24578: Keysym = Keysym(0x010028da);
    pub const braille_dots_124578: Keysym = Keysym(0x010028db);
    pub const braille_dots_34578: Keysym = Keysym(0x010028dc);
    pub const braille_dots_134578: Keysym = Keysym(0x010028dd);
    pub const braille_dots_234578: Keysym = Keysym(0x010028de);
    pub const braille_dots_1234578: Keysym = Keysym(0x010028df);
    pub const braille_dots_678: Keysym = Keysym(0x010028e0);
    pub const braille_dots_1678: Keysym = Keysym(0x010028e1);
    pub const braille_dots_2678: Keysym = Keysym(0x010028e2);
    pub const braille_dots_12678: Keysym = Keysym(0x010028e3);
    pub const braille_dots_3678: Keysym = Keysym(0x010028e4);
    pub const braille_dots_13678: Keysym = Keysym(0x010028e5);
    pub const braille_dots_23678: Keysym = Keysym(0x010028e6);
    pub const braille_dots_123678: Keysym = Keysym(0x010028e7);
    pub const braille_dots_4678: Keysym = Keysym(0x010028e8);
    pub const braille_dots_14678: Keysym = Keysym(0x010028e9);
    pub const braille_dots_24678: Keysym = Keysym(0x010028ea);
    pub const braille_dots_124678: Keysym = Keysym(0x010028eb);
    pub const braille_dots_34678: Keysym = Keysym(0x010028ec);
    pub const braille_dots_134678: Keysym = Keysym(0x010028ed);
    pub const braille_dots_234678: Keysym = Keysym(0x010028ee);
    pub const braille_dots_1234678: Keysym = Keysym(0x010028ef);
    pub const braille_dots_5678: Keysym = Keysym(0x010028f0);
    pub const braille_dots_15678: Keysym = Keysym(0x010028f1);
    pub const braille_dots_25678: Keysym = Keysym(0x010028f2);
    pub const braille_dots_125678: Keysym = Keysym(0x010028f3);
    pub const braille_dots_35678: Keysym = Keysym(0x010028f4);
    pub const braille_dots_135678: Keysym = Keysym(0x010028f5);
    pub const braille_dots_235678: Keysym = Keysym(0x010028f6);
    pub const braille_dots_1235678: Keysym = Keysym(0x010028f7);
    pub const braille_dots_45678: Keysym = Keysym(0x010028f8);
    pub const braille_dots_145678: Keysym = Keysym(0x010028f9);
    pub const braille_dots_245678: Keysym = Keysym(0x010028fa);
    pub const braille_dots_1245678: Keysym = Keysym(0x010028fb);
    pub const braille_dots_345678: Keysym = Keysym(0x010028fc);
    pub const braille_dots_1345678: Keysym = Keysym(0x010028fd);
    pub const braille_dots_2345678: Keysym = Keysym(0x010028fe);
    pub const braille_dots_12345678: Keysym = Keysym(0x010028ff);
    pub const hpmute_acute: Keysym = Keysym(0x100000a8);
    pub const mute_acute: Keysym = Keysym(0x100000a8);
    pub const hpmute_grave: Keysym = Keysym(0x100000a9);
    pub const mute_grave: Keysym = Keysym(0x100000a9);
    pub const hpmute_asciicircum: Keysym = Keysym(0x100000aa);
    pub const mute_asciicircum: Keysym = Keysym(0x100000aa);
    pub const hpmute_diaeresis: Keysym = Keysym(0x100000ab);
    pub const mute_diaeresis: Keysym = Keysym(0x100000ab);
    pub const hpmute_asciitilde: Keysym = Keysym(0x100000ac);
    pub const mute_asciitilde: Keysym = Keysym(0x100000ac);
    pub const hplira: Keysym = Keysym(0x100000af);
    pub const lira: Keysym = Keysym(0x100000af);
    pub const hpguilder: Keysym = Keysym(0x100000be);
    pub const guilder: Keysym = Keysym(0x100000be);
    pub const hpYdiaeresis: Keysym = Keysym(0x100000ee);
    pub const hpIO: Keysym = Keysym(0x100000ee);
    pub const IO: Keysym = Keysym(0x100000ee);
    pub const hplongminus: Keysym = Keysym(0x100000f6);
    pub const longminus: Keysym = Keysym(0x100000f6);
    pub const hpblock: Keysym = Keysym(0x100000fc);
    pub const block: Keysym = Keysym(0x100000fc);
    pub const Ddiaeresis: Keysym = Keysym(0x1000fe22);
    pub const Dacute_accent: Keysym = Keysym(0x1000fe27);
    pub const Dcedilla_accent: Keysym = Keysym(0x1000fe2c);
    pub const Dcircumflex_accent: Keysym = Keysym(0x1000fe5e);
    pub const Dgrave_accent: Keysym = Keysym(0x1000fe60);
    pub const Dtilde: Keysym = Keysym(0x1000fe7e);
    pub const Dring_accent: Keysym = Keysym(0x1000feb0);
    pub const DRemove: Keysym = Keysym(0x1000ff00);
    pub const hpModelock1: Keysym = Keysym(0x1000ff48);
    pub const hpModelock2: Keysym = Keysym(0x1000ff49);
    pub const hpReset: Keysym = Keysym(0x1000ff6c);
    pub const Reset: Keysym = Keysym(0x1000ff6c);
    pub const hpSystem: Keysym = Keysym(0x1000ff6d);
    pub const System: Keysym = Keysym(0x1000ff6d);
    pub const hpUser: Keysym = Keysym(0x1000ff6e);
    pub const User: Keysym = Keysym(0x1000ff6e);
    pub const hpClearLine: Keysym = Keysym(0x1000ff6f);
    pub const ClearLine: Keysym = Keysym(0x1000ff6f);
    pub const hpInsertLine: Keysym = Keysym(0x1000ff70);
    pub const InsertLine: Keysym = Keysym(0x1000ff70);
    pub const hpDeleteLine: Keysym = Keysym(0x1000ff71);
    pub const DeleteLine: Keysym = Keysym(0x1000ff71);
    pub const hpInsertChar: Keysym = Keysym(0x1000ff72);
    pub const InsertChar: Keysym = Keysym(0x1000ff72);
    pub const hpDeleteChar: Keysym = Keysym(0x1000ff73);
    pub const DeleteChar: Keysym = Keysym(0x1000ff73);
    pub const hpBackTab: Keysym = Keysym(0x1000ff74);
    pub const BackTab: Keysym = Keysym(0x1000ff74);
    pub const hpKP_BackTab: Keysym = Keysym(0x1000ff75);
    pub const KP_BackTab: Keysym = Keysym(0x1000ff75);
    pub const Ext16bit_L: Keysym = Keysym(0x1000ff76);
    pub const Ext16bit_R: Keysym = Keysym(0x1000ff77);
    pub const osfCopy: Keysym = Keysym(0x1004ff02);
    pub const osfCut: Keysym = Keysym(0x1004ff03);
    pub const osfPaste: Keysym = Keysym(0x1004ff04);
    pub const osfBackTab: Keysym = Keysym(0x1004ff07);
    pub const osfBackSpace: Keysym = Keysym(0x1004ff08);
    pub const osfClear: Keysym = Keysym(0x1004ff0b);
    pub const osfEscape: Keysym = Keysym(0x1004ff1b);
    pub const osfAddMode: Keysym = Keysym(0x1004ff31);
    pub const osfPrimaryPaste: Keysym = Keysym(0x1004ff32);
    pub const osfQuickPaste: Keysym = Keysym(0x1004ff33);
    pub const osfPageLeft: Keysym = Keysym(0x1004ff40);
    pub const osfPageUp: Keysym = Keysym(0x1004ff41);
    pub const osfPageDown: Keysym = Keysym(0x1004ff42);
    pub const osfPageRight: Keysym = Keysym(0x1004ff43);
    pub const osfActivate: Keysym = Keysym(0x1004ff44);
    pub const osfMenuBar: Keysym = Keysym(0x1004ff45);
    pub const osfLeft: Keysym = Keysym(0x1004ff51);
    pub const osfUp: Keysym = Keysym(0x1004ff52);
    pub const osfRight: Keysym = Keysym(0x1004ff53);
    pub const osfDown: Keysym = Keysym(0x1004ff54);
    pub const osfEndLine: Keysym = Keysym(0x1004ff57);
    pub const osfBeginLine: Keysym = Keysym(0x1004ff58);
    pub const osfEndData: Keysym = Keysym(0x1004ff59);
    pub const osfBeginData: Keysym = Keysym(0x1004ff5a);
    pub const osfPrevMenu: Keysym = Keysym(0x1004ff5b);
    pub const osfNextMenu: Keysym = Keysym(0x1004ff5c);
    pub const osfPrevField: Keysym = Keysym(0x1004ff5d);
    pub const osfNextField: Keysym = Keysym(0x1004ff5e);
    pub const osfSelect: Keysym = Keysym(0x1004ff60);
    pub const osfInsert: Keysym = Keysym(0x1004ff63);
    pub const osfUndo: Keysym = Keysym(0x1004ff65);
    pub const osfMenu: Keysym = Keysym(0x1004ff67);
    pub const osfCancel: Keysym = Keysym(0x1004ff69);
    pub const osfHelp: Keysym = Keysym(0x1004ff6a);
    pub const osfSelectAll: Keysym = Keysym(0x1004ff71);
    pub const osfDeselectAll: Keysym = Keysym(0x1004ff72);
    pub const osfReselect: Keysym = Keysym(0x1004ff73);
    pub const osfExtend: Keysym = Keysym(0x1004ff74);
    pub const osfRestore: Keysym = Keysym(0x1004ff78);
    pub const osfDelete: Keysym = Keysym(0x1004ffff);
    pub const SunFA_Grave: Keysym = Keysym(0x1005ff00);
    pub const SunFA_Circum: Keysym = Keysym(0x1005ff01);
    pub const SunFA_Tilde: Keysym = Keysym(0x1005ff02);
    pub const SunFA_Acute: Keysym = Keysym(0x1005ff03);
    pub const SunFA_Diaeresis: Keysym = Keysym(0x1005ff04);
    pub const SunFA_Cedilla: Keysym = Keysym(0x1005ff05);
    pub const SunF36: Keysym = Keysym(0x1005ff10);
    pub const SunF37: Keysym = Keysym(0x1005ff11);
    pub const SunSys_Req: Keysym = Keysym(0x1005ff60);
    pub const SunProps: Keysym = Keysym(0x1005ff70);
    pub const SunFront: Keysym = Keysym(0x1005ff71);
    pub const SunCopy: Keysym = Keysym(0x1005ff72);
    pub const SunOpen: Keysym = Keysym(0x1005ff73);
    pub const SunPaste: Keysym = Keysym(0x1005ff74);
    pub const SunCut: Keysym = Keysym(0x1005ff75);
    pub const SunPowerSwitch: Keysym = Keysym(0x1005ff76);
    pub const SunAudioLowerVolume: Keysym = Keysym(0x1005ff77);
    pub const SunAudioMute: Keysym = Keysym(0x1005ff78);
    pub const SunAudioRaiseVolume: Keysym = Keysym(0x1005ff79);
    pub const SunVideoDegauss: Keysym = Keysym(0x1005ff7a);
    pub const SunVideoLowerBrightness: Keysym = Keysym(0x1005ff7b);
    pub const SunVideoRaiseBrightness: Keysym = Keysym(0x1005ff7c);
    pub const SunPowerSwitchShift: Keysym = Keysym(0x1005ff7d);
    pub const XF86MediaPlayPause: Keysym = Keysym(0x100810a4);
    pub const XF86Exit: Keysym = Keysym(0x100810ae);
    pub const XF86AudioBassBoost: Keysym = Keysym(0x100810d1);
    pub const XF86Sport: Keysym = Keysym(0x100810dc);
    pub const XF86BrightnessAuto: Keysym = Keysym(0x100810f4);
    pub const XF86MonBrightnessAuto: Keysym = Keysym(0x100810f4);
    pub const XF86DisplayOff: Keysym = Keysym(0x100810f5);
    pub const XF86OK: Keysym = Keysym(0x10081160);
    pub const XF86GoTo: Keysym = Keysym(0x10081162);
    pub const XF86Info: Keysym = Keysym(0x10081166);
    pub const XF86VendorLogo: Keysym = Keysym(0x10081168);
    pub const XF86MediaSelectProgramGuide: Keysym = Keysym(0x1008116a);
    pub const XF86MediaSelectHome: Keysym = Keysym(0x1008116e);
    pub const XF86MediaLanguageMenu: Keysym = Keysym(0x10081170);
    pub const XF86MediaTitleMenu: Keysym = Keysym(0x10081171);
    pub const XF86AudioChannelMode: Keysym = Keysym(0x10081175);
    pub const XF86AspectRatio: Keysym = Keysym(0x10081177);
    pub const XF86MediaSelectPC: Keysym = Keysym(0x10081178);
    pub const XF86MediaSelectTV: Keysym = Keysym(0x10081179);
    pub const XF86MediaSelectCable: Keysym = Keysym(0x1008117a);
    pub const XF86MediaSelectVCR: Keysym = Keysym(0x1008117b);
    pub const XF86MediaSelectVCRPlus: Keysym = Keysym(0x1008117c);
    pub const XF86MediaSelectSatellite: Keysym = Keysym(0x1008117d);
    pub const XF86MediaSelectTape: Keysym = Keysym(0x10081180);
    pub const XF86MediaSelectRadio: Keysym = Keysym(0x10081181);
    pub const XF86MediaSelectTuner: Keysym = Keysym(0x10081182);
    pub const XF86MediaPlayer: Keysym = Keysym(0x10081183);
    pub const XF86MediaSelectTeletext: Keysym = Keysym(0x10081184);
    pub const XF86DVD: Keysym = Keysym(0x10081185);
    pub const XF86MediaSelectDVD: Keysym = Keysym(0x10081185);
    pub const XF86MediaSelectAuxiliary: Keysym = Keysym(0x10081186);
    pub const XF86Audio: Keysym = Keysym(0x10081188);
    pub const XF86ChannelUp: Keysym = Keysym(0x10081192);
    pub const XF86ChannelDown: Keysym = Keysym(0x10081193);
    pub const XF86MediaPlaySlow: Keysym = Keysym(0x10081199);
    pub const XF86Break: Keysym = Keysym(0x1008119b);
    pub const XF86NumberEntryMode: Keysym = Keysym(0x1008119d);
    pub const XF86VideoPhone: Keysym = Keysym(0x100811a0);
    pub const XF86ZoomReset: Keysym = Keysym(0x100811a4);
    pub const XF86Editor: Keysym = Keysym(0x100811a6);
    pub const XF86GraphicsEditor: Keysym = Keysym(0x100811a8);
    pub const XF86Presentation: Keysym = Keysym(0x100811a9);
    pub const XF86Database: Keysym = Keysym(0x100811aa);
    pub const XF86Voicemail: Keysym = Keysym(0x100811ac);
    pub const XF86Addressbook: Keysym = Keysym(0x100811ad);
    pub const XF86DisplayToggle: Keysym = Keysym(0x100811af);
    pub const XF86SpellCheck: Keysym = Keysym(0x100811b0);
    pub const XF86ContextMenu: Keysym = Keysym(0x100811b6);
    pub const XF86MediaRepeat: Keysym = Keysym(0x100811b7);
    pub const XF8610ChannelsUp: Keysym = Keysym(0x100811b8);
    pub const XF8610ChannelsDown: Keysym = Keysym(0x100811b9);
    pub const XF86Images: Keysym = Keysym(0x100811ba);
    pub const XF86NotificationCenter: Keysym = Keysym(0x100811bc);
    pub const XF86PickupPhone: Keysym = Keysym(0x100811bd);
    pub const XF86HangupPhone: Keysym = Keysym(0x100811be);
    pub const XF86LinkPhone: Keysym = Keysym(0x100811bf);
    pub const XF86Fn: Keysym = Keysym(0x100811d0);
    pub const XF86Fn_Esc: Keysym = Keysym(0x100811d1);
    pub const XF86Fn_F1: Keysym = Keysym(0x100811d2);
    pub const XF86Fn_F2: Keysym = Keysym(0x100811d3);
    pub const XF86Fn_F3: Keysym = Keysym(0x100811d4);
    pub const XF86Fn_F4: Keysym = Keysym(0x100811d5);
    pub const XF86Fn_F5: Keysym = Keysym(0x100811d6);
    pub const XF86Fn_F6: Keysym = Keysym(0x100811d7);
    pub const XF86Fn_F7: Keysym = Keysym(0x100811d8);
    pub const XF86Fn_F8: Keysym = Keysym(0x100811d9);
    pub const XF86Fn_F9: Keysym = Keysym(0x100811da);
    pub const XF86Fn_F10: Keysym = Keysym(0x100811db);
    pub const XF86Fn_F11: Keysym = Keysym(0x100811dc);
    pub const XF86Fn_F12: Keysym = Keysym(0x100811dd);
    pub const XF86Fn_1: Keysym = Keysym(0x100811de);
    pub const XF86Fn_2: Keysym = Keysym(0x100811df);
    pub const XF86Fn_D: Keysym = Keysym(0x100811e0);
    pub const XF86Fn_E: Keysym = Keysym(0x100811e1);
    pub const XF86Fn_F: Keysym = Keysym(0x100811e2);
    pub const XF86Fn_S: Keysym = Keysym(0x100811e3);
    pub const XF86Fn_B: Keysym = Keysym(0x100811e4);
    pub const XF86FnRightShift: Keysym = Keysym(0x100811e5);
    pub const XF86Numeric0: Keysym = Keysym(0x10081200);
    pub const XF86Numeric1: Keysym = Keysym(0x10081201);
    pub const XF86Numeric2: Keysym = Keysym(0x10081202);
    pub const XF86Numeric3: Keysym = Keysym(0x10081203);
    pub const XF86Numeric4: Keysym = Keysym(0x10081204);
    pub const XF86Numeric5: Keysym = Keysym(0x10081205);
    pub const XF86Numeric6: Keysym = Keysym(0x10081206);
    pub const XF86Numeric7: Keysym = Keysym(0x10081207);
    pub const XF86Numeric8: Keysym = Keysym(0x10081208);
    pub const XF86Numeric9: Keysym = Keysym(0x10081209);
    pub const XF86NumericStar: Keysym = Keysym(0x1008120a);
    pub const XF86NumericPound: Keysym = Keysym(0x1008120b);
    pub const XF86NumericA: Keysym = Keysym(0x1008120c);
    pub const XF86NumericB: Keysym = Keysym(0x1008120d);
    pub const XF86NumericC: Keysym = Keysym(0x1008120e);
    pub const XF86NumericD: Keysym = Keysym(0x1008120f);
    pub const XF86CameraFocus: Keysym = Keysym(0x10081210);
    pub const XF86WPSButton: Keysym = Keysym(0x10081211);
    pub const XF86CameraZoomIn: Keysym = Keysym(0x10081215);
    pub const XF86CameraZoomOut: Keysym = Keysym(0x10081216);
    pub const XF86CameraUp: Keysym = Keysym(0x10081217);
    pub const XF86CameraDown: Keysym = Keysym(0x10081218);
    pub const XF86CameraLeft: Keysym = Keysym(0x10081219);
    pub const XF86CameraRight: Keysym = Keysym(0x1008121a);
    pub const XF86AttendantOn: Keysym = Keysym(0x1008121b);
    pub const XF86AttendantOff: Keysym = Keysym(0x1008121c);
    pub const XF86AttendantToggle: Keysym = Keysym(0x1008121d);
    pub const XF86LightsToggle: Keysym = Keysym(0x1008121e);
    pub const XF86ALSToggle: Keysym = Keysym(0x10081230);
    pub const XF86RefreshRateToggle: Keysym = Keysym(0x10081232);
    pub const XF86Buttonconfig: Keysym = Keysym(0x10081240);
    pub const XF86Taskmanager: Keysym = Keysym(0x10081241);
    pub const XF86Journal: Keysym = Keysym(0x10081242);
    pub const XF86ControlPanel: Keysym = Keysym(0x10081243);
    pub const XF86AppSelect: Keysym = Keysym(0x10081244);
    pub const XF86Screensaver: Keysym = Keysym(0x10081245);
    pub const XF86VoiceCommand: Keysym = Keysym(0x10081246);
    pub const XF86Assistant: Keysym = Keysym(0x10081247);
    pub const XF86EmojiPicker: Keysym = Keysym(0x10081249);
    pub const XF86Dictate: Keysym = Keysym(0x1008124a);
    pub const XF86CameraAccessEnable: Keysym = Keysym(0x1008124b);
    pub const XF86CameraAccessDisable: Keysym = Keysym(0x1008124c);
    pub const XF86CameraAccessToggle: Keysym = Keysym(0x1008124d);
    pub const XF86Accessibility: Keysym = Keysym(0x1008124e);
    pub const XF86DoNotDisturb: Keysym = Keysym(0x1008124f);
    pub const XF86BrightnessMin: Keysym = Keysym(0x10081250);
    pub const XF86BrightnessMax: Keysym = Keysym(0x10081251);
    pub const XF86ElectronicPrivacyScreenOn: Keysym = Keysym(0x10081252);
    pub const XF86ElectronicPrivacyScreenOff: Keysym = Keysym(0x10081253);
    pub const XF86ActionOnSelection: Keysym = Keysym(0x10081254);
    pub const XF86ContextualInsert: Keysym = Keysym(0x10081255);
    pub const XF86ContextualQuery: Keysym = Keysym(0x10081256);
    pub const XF86KbdInputAssistPrev: Keysym = Keysym(0x10081260);
    pub const XF86KbdInputAssistNext: Keysym = Keysym(0x10081261);
    pub const XF86KbdInputAssistPrevgroup: Keysym = Keysym(0x10081262);
    pub const XF86KbdInputAssistNextgroup: Keysym = Keysym(0x10081263);
    pub const XF86KbdInputAssistAccept: Keysym = Keysym(0x10081264);
    pub const XF86KbdInputAssistCancel: Keysym = Keysym(0x10081265);
    pub const XF86RightUp: Keysym = Keysym(0x10081266);
    pub const XF86RightDown: Keysym = Keysym(0x10081267);
    pub const XF86LeftUp: Keysym = Keysym(0x10081268);
    pub const XF86LeftDown: Keysym = Keysym(0x10081269);
    pub const XF86RootMenu: Keysym = Keysym(0x1008126a);
    pub const XF86MediaTopMenu: Keysym = Keysym(0x1008126b);
    pub const XF86Numeric11: Keysym = Keysym(0x1008126c);
    pub const XF86Numeric12: Keysym = Keysym(0x1008126d);
    pub const XF86AudioDesc: Keysym = Keysym(0x1008126e);
    pub const XF863DMode: Keysym = Keysym(0x1008126f);
    pub const XF86NextFavorite: Keysym = Keysym(0x10081270);
    pub const XF86StopRecord: Keysym = Keysym(0x10081271);
    pub const XF86PauseRecord: Keysym = Keysym(0x10081272);
    pub const XF86VOD: Keysym = Keysym(0x10081273);
    pub const XF86Unmute: Keysym = Keysym(0x10081274);
    pub const XF86FastReverse: Keysym = Keysym(0x10081275);
    pub const XF86SlowReverse: Keysym = Keysym(0x10081276);
    pub const XF86Data: Keysym = Keysym(0x10081277);
    pub const XF86OnScreenKeyboard: Keysym = Keysym(0x10081278);
    pub const XF86PrivacyScreenToggle: Keysym = Keysym(0x10081279);
    pub const XF86SelectiveScreenshot: Keysym = Keysym(0x1008127a);
    pub const XF86NextElement: Keysym = Keysym(0x1008127b);
    pub const XF86PreviousElement: Keysym = Keysym(0x1008127c);
    pub const XF86AutopilotEngageToggle: Keysym = Keysym(0x1008127d);
    pub const XF86MarkWaypoint: Keysym = Keysym(0x1008127e);
    pub const XF86Sos: Keysym = Keysym(0x1008127f);
    pub const XF86NavChart: Keysym = Keysym(0x10081280);
    pub const XF86FishingChart: Keysym = Keysym(0x10081281);
    pub const XF86SingleRangeRadar: Keysym = Keysym(0x10081282);
    pub const XF86DualRangeRadar: Keysym = Keysym(0x10081283);
    pub const XF86RadarOverlay: Keysym = Keysym(0x10081284);
    pub const XF86TraditionalSonar: Keysym = Keysym(0x10081285);
    pub const XF86ClearvuSonar: Keysym = Keysym(0x10081286);
    pub const XF86SidevuSonar: Keysym = Keysym(0x10081287);
    pub const XF86NavInfo: Keysym = Keysym(0x10081288);
    pub const XF86Macro1: Keysym = Keysym(0x10081290);
    pub const XF86Macro2: Keysym = Keysym(0x10081291);
    pub const XF86Macro3: Keysym = Keysym(0x10081292);
    pub const XF86Macro4: Keysym = Keysym(0x10081293);
    pub const XF86Macro5: Keysym = Keysym(0x10081294);
    pub const XF86Macro6: Keysym = Keysym(0x10081295);
    pub const XF86Macro7: Keysym = Keysym(0x10081296);
    pub const XF86Macro8: Keysym = Keysym(0x10081297);
    pub const XF86Macro9: Keysym = Keysym(0x10081298);
    pub const XF86Macro10: Keysym = Keysym(0x10081299);
    pub const XF86Macro11: Keysym = Keysym(0x1008129a);
    pub const XF86Macro12: Keysym = Keysym(0x1008129b);
    pub const XF86Macro13: Keysym = Keysym(0x1008129c);
    pub const XF86Macro14: Keysym = Keysym(0x1008129d);
    pub const XF86Macro15: Keysym = Keysym(0x1008129e);
    pub const XF86Macro16: Keysym = Keysym(0x1008129f);
    pub const XF86Macro17: Keysym = Keysym(0x100812a0);
    pub const XF86Macro18: Keysym = Keysym(0x100812a1);
    pub const XF86Macro19: Keysym = Keysym(0x100812a2);
    pub const XF86Macro20: Keysym = Keysym(0x100812a3);
    pub const XF86Macro21: Keysym = Keysym(0x100812a4);
    pub const XF86Macro22: Keysym = Keysym(0x100812a5);
    pub const XF86Macro23: Keysym = Keysym(0x100812a6);
    pub const XF86Macro24: Keysym = Keysym(0x100812a7);
    pub const XF86Macro25: Keysym = Keysym(0x100812a8);
    pub const XF86Macro26: Keysym = Keysym(0x100812a9);
    pub const XF86Macro27: Keysym = Keysym(0x100812aa);
    pub const XF86Macro28: Keysym = Keysym(0x100812ab);
    pub const XF86Macro29: Keysym = Keysym(0x100812ac);
    pub const XF86Macro30: Keysym = Keysym(0x100812ad);
    pub const XF86MacroRecordStart: Keysym = Keysym(0x100812b0);
    pub const XF86MacroRecordStop: Keysym = Keysym(0x100812b1);
    pub const XF86MacroPresetCycle: Keysym = Keysym(0x100812b2);
    pub const XF86MacroPreset1: Keysym = Keysym(0x100812b3);
    pub const XF86MacroPreset2: Keysym = Keysym(0x100812b4);
    pub const XF86MacroPreset3: Keysym = Keysym(0x100812b5);
    pub const XF86KbdLcdMenu1: Keysym = Keysym(0x100812b8);
    pub const XF86KbdLcdMenu2: Keysym = Keysym(0x100812b9);
    pub const XF86KbdLcdMenu3: Keysym = Keysym(0x100812ba);
    pub const XF86KbdLcdMenu4: Keysym = Keysym(0x100812bb);
    pub const XF86KbdLcdMenu5: Keysym = Keysym(0x100812bc);
    pub const XF86PerformanceMode: Keysym = Keysym(0x100812bd);
    pub const XF86Switch_VT_1: Keysym = Keysym(0x1008fe01);
    pub const XF86Switch_VT_2: Keysym = Keysym(0x1008fe02);
    pub const XF86Switch_VT_3: Keysym = Keysym(0x1008fe03);
    pub const XF86Switch_VT_4: Keysym = Keysym(0x1008fe04);
    pub const XF86Switch_VT_5: Keysym = Keysym(0x1008fe05);
    pub const XF86Switch_VT_6: Keysym = Keysym(0x1008fe06);
    pub const XF86Switch_VT_7: Keysym = Keysym(0x1008fe07);
    pub const XF86Switch_VT_8: Keysym = Keysym(0x1008fe08);
    pub const XF86Switch_VT_9: Keysym = Keysym(0x1008fe09);
    pub const XF86Switch_VT_10: Keysym = Keysym(0x1008fe0a);
    pub const XF86Switch_VT_11: Keysym = Keysym(0x1008fe0b);
    pub const XF86Switch_VT_12: Keysym = Keysym(0x1008fe0c);
    pub const XF86Ungrab: Keysym = Keysym(0x1008fe20);
    pub const XF86ClearGrab: Keysym = Keysym(0x1008fe21);
    pub const XF86Next_VMode: Keysym = Keysym(0x1008fe22);
    pub const XF86Prev_VMode: Keysym = Keysym(0x1008fe23);
    pub const XF86LogWindowTree: Keysym = Keysym(0x1008fe24);
    pub const XF86LogGrabInfo: Keysym = Keysym(0x1008fe25);
    pub const XF86ModeLock: Keysym = Keysym(0x1008ff01);
    pub const XF86MonBrightnessUp: Keysym = Keysym(0x1008ff02);
    pub const XF86MonBrightnessDown: Keysym = Keysym(0x1008ff03);
    pub const XF86KbdLightOnOff: Keysym = Keysym(0x1008ff04);
    pub const XF86KbdBrightnessUp: Keysym = Keysym(0x1008ff05);
    pub const XF86KbdBrightnessDown: Keysym = Keysym(0x1008ff06);
    pub const XF86MonBrightnessCycle: Keysym = Keysym(0x1008ff07);
    pub const XF86Standby: Keysym = Keysym(0x1008ff10);
    pub const XF86AudioLowerVolume: Keysym = Keysym(0x1008ff11);
    pub const XF86AudioMute: Keysym = Keysym(0x1008ff12);
    pub const XF86AudioRaiseVolume: Keysym = Keysym(0x1008ff13);
    pub const XF86AudioPlay: Keysym = Keysym(0x1008ff14);
    pub const XF86AudioStop: Keysym = Keysym(0x1008ff15);
    pub const XF86AudioPrev: Keysym = Keysym(0x1008ff16);
    pub const XF86AudioNext: Keysym = Keysym(0x1008ff17);
    pub const XF86HomePage: Keysym = Keysym(0x1008ff18);
    pub const XF86Mail: Keysym = Keysym(0x1008ff19);
    pub const XF86Start: Keysym = Keysym(0x1008ff1a);
    pub const XF86Search: Keysym = Keysym(0x1008ff1b);
    pub const XF86AudioRecord: Keysym = Keysym(0x1008ff1c);
    pub const XF86Calculator: Keysym = Keysym(0x1008ff1d);
    pub const XF86Memo: Keysym = Keysym(0x1008ff1e);
    pub const XF86ToDoList: Keysym = Keysym(0x1008ff1f);
    pub const XF86Calendar: Keysym = Keysym(0x1008ff20);
    pub const XF86PowerDown: Keysym = Keysym(0x1008ff21);
    pub const XF86ContrastAdjust: Keysym = Keysym(0x1008ff22);
    pub const XF86RockerUp: Keysym = Keysym(0x1008ff23);
    pub const XF86RockerDown: Keysym = Keysym(0x1008ff24);
    pub const XF86RockerEnter: Keysym = Keysym(0x1008ff25);
    pub const XF86Back: Keysym = Keysym(0x1008ff26);
    pub const XF86Forward: Keysym = Keysym(0x1008ff27);
    pub const XF86Stop: Keysym = Keysym(0x1008ff28);
    pub const XF86Refresh: Keysym = Keysym(0x1008ff29);
    pub const XF86PowerOff: Keysym = Keysym(0x1008ff2a);
    pub const XF86WakeUp: Keysym = Keysym(0x1008ff2b);
    pub const XF86Eject: Keysym = Keysym(0x1008ff2c);
    pub const XF86ScreenSaver: Keysym = Keysym(0x1008ff2d);
    pub const XF86WWW: Keysym = Keysym(0x1008ff2e);
    pub const XF86Sleep: Keysym = Keysym(0x1008ff2f);
    pub const XF86Favorites: Keysym = Keysym(0x1008ff30);
    pub const XF86AudioPause: Keysym = Keysym(0x1008ff31);
    pub const XF86AudioMedia: Keysym = Keysym(0x1008ff32);
    pub const XF86MyComputer: Keysym = Keysym(0x1008ff33);
    pub const XF86VendorHome: Keysym = Keysym(0x1008ff34);
    pub const XF86LightBulb: Keysym = Keysym(0x1008ff35);
    pub const XF86Shop: Keysym = Keysym(0x1008ff36);
    pub const XF86History: Keysym = Keysym(0x1008ff37);
    pub const XF86OpenURL: Keysym = Keysym(0x1008ff38);
    pub const XF86AddFavorite: Keysym = Keysym(0x1008ff39);
    pub const XF86HotLinks: Keysym = Keysym(0x1008ff3a);
    pub const XF86BrightnessAdjust: Keysym = Keysym(0x1008ff3b);
    pub const XF86Finance: Keysym = Keysym(0x1008ff3c);
    pub const XF86Community: Keysym = Keysym(0x1008ff3d);
    pub const XF86AudioRewind: Keysym = Keysym(0x1008ff3e);
    pub const XF86BackForward: Keysym = Keysym(0x1008ff3f);
    pub const XF86Launch0: Keysym = Keysym(0x1008ff40);
    pub const XF86Launch1: Keysym = Keysym(0x1008ff41);
    pub const XF86Launch2: Keysym = Keysym(0x1008ff42);
    pub const XF86Launch3: Keysym = Keysym(0x1008ff43);
    pub const XF86Launch4: Keysym = Keysym(0x1008ff44);
    pub const XF86Launch5: Keysym = Keysym(0x1008ff45);
    pub const XF86Launch6: Keysym = Keysym(0x1008ff46);
    pub const XF86Launch7: Keysym = Keysym(0x1008ff47);
    pub const XF86Launch8: Keysym = Keysym(0x1008ff48);
    pub const XF86Launch9: Keysym = Keysym(0x1008ff49);
    pub const XF86LaunchA: Keysym = Keysym(0x1008ff4a);
    pub const XF86LaunchB: Keysym = Keysym(0x1008ff4b);
    pub const XF86LaunchC: Keysym = Keysym(0x1008ff4c);
    pub const XF86LaunchD: Keysym = Keysym(0x1008ff4d);
    pub const XF86LaunchE: Keysym = Keysym(0x1008ff4e);
    pub const XF86LaunchF: Keysym = Keysym(0x1008ff4f);
    pub const XF86ApplicationLeft: Keysym = Keysym(0x1008ff50);
    pub const XF86ApplicationRight: Keysym = Keysym(0x1008ff51);
    pub const XF86Book: Keysym = Keysym(0x1008ff52);
    pub const XF86CD: Keysym = Keysym(0x1008ff53);
    pub const XF86MediaSelectCD: Keysym = Keysym(0x1008ff53);
    pub const XF86Calculater: Keysym = Keysym(0x1008ff54);
    pub const XF86Clear: Keysym = Keysym(0x1008ff55);
    pub const XF86Close: Keysym = Keysym(0x1008ff56);
    pub const XF86Copy: Keysym = Keysym(0x1008ff57);
    pub const XF86Cut: Keysym = Keysym(0x1008ff58);
    pub const XF86Display: Keysym = Keysym(0x1008ff59);
    pub const XF86DOS: Keysym = Keysym(0x1008ff5a);
    pub const XF86Documents: Keysym = Keysym(0x1008ff5b);
    pub const XF86Excel: Keysym = Keysym(0x1008ff5c);
    pub const XF86Explorer: Keysym = Keysym(0x1008ff5d);
    pub const XF86Game: Keysym = Keysym(0x1008ff5e);
    pub const XF86Go: Keysym = Keysym(0x1008ff5f);
    pub const XF86iTouch: Keysym = Keysym(0x1008ff60);
    pub const XF86LogOff: Keysym = Keysym(0x1008ff61);
    pub const XF86Market: Keysym = Keysym(0x1008ff62);
    pub const XF86Meeting: Keysym = Keysym(0x1008ff63);
    pub const XF86MenuKB: Keysym = Keysym(0x1008ff65);
    pub const XF86MenuPB: Keysym = Keysym(0x1008ff66);
    pub const XF86MySites: Keysym = Keysym(0x1008ff67);
    pub const XF86New: Keysym = Keysym(0x1008ff68);
    pub const XF86News: Keysym = Keysym(0x1008ff69);
    pub const XF86OfficeHome: Keysym = Keysym(0x1008ff6a);
    pub const XF86Open: Keysym = Keysym(0x1008ff6b);
    pub const XF86Option: Keysym = Keysym(0x1008ff6c);
    pub const XF86Paste: Keysym = Keysym(0x1008ff6d);
    pub const XF86Phone: Keysym = Keysym(0x1008ff6e);
    pub const XF86Q: Keysym = Keysym(0x1008ff70);
    pub const XF86Reply: Keysym = Keysym(0x1008ff72);
    pub const XF86Reload: Keysym = Keysym(0x1008ff73);
    pub const XF86RotateWindows: Keysym = Keysym(0x1008ff74);
    pub const XF86RotationPB: Keysym = Keysym(0x1008ff75);
    pub const XF86RotationKB: Keysym = Keysym(0x1008ff76);
    pub const XF86Save: Keysym = Keysym(0x1008ff77);
    pub const XF86ScrollUp: Keysym = Keysym(0x1008ff78);
    pub const XF86ScrollDown: Keysym = Keysym(0x1008ff79);
    pub const XF86ScrollClick: Keysym = Keysym(0x1008ff7a);
    pub const XF86Send: Keysym = Keysym(0x1008ff7b);
    pub const XF86Spell: Keysym = Keysym(0x1008ff7c);
    pub const XF86SplitScreen: Keysym = Keysym(0x1008ff7d);
    pub const XF86Support: Keysym = Keysym(0x1008ff7e);
    pub const XF86TaskPane: Keysym = Keysym(0x1008ff7f);
    pub const XF86Terminal: Keysym = Keysym(0x1008ff80);
    pub const XF86Tools: Keysym = Keysym(0x1008ff81);
    pub const XF86Travel: Keysym = Keysym(0x1008ff82);
    pub const XF86UserPB: Keysym = Keysym(0x1008ff84);
    pub const XF86User1KB: Keysym = Keysym(0x1008ff85);
    pub const XF86User2KB: Keysym = Keysym(0x1008ff86);
    pub const XF86Video: Keysym = Keysym(0x1008ff87);
    pub const XF86WheelButton: Keysym = Keysym(0x1008ff88);
    pub const XF86Word: Keysym = Keysym(0x1008ff89);
    pub const XF86Xfer: Keysym = Keysym(0x1008ff8a);
    pub const XF86ZoomIn: Keysym = Keysym(0x1008ff8b);
    pub const XF86ZoomOut: Keysym = Keysym(0x1008ff8c);
    pub const XF86Away: Keysym = Keysym(0x1008ff8d);
    pub const XF86Messenger: Keysym = Keysym(0x1008ff8e);
    pub const XF86WebCam: Keysym = Keysym(0x1008ff8f);
    pub const XF86MailForward: Keysym = Keysym(0x1008ff90);
    pub const XF86Pictures: Keysym = Keysym(0x1008ff91);
    pub const XF86Music: Keysym = Keysym(0x1008ff92);
    pub const XF86Battery: Keysym = Keysym(0x1008ff93);
    pub const XF86Bluetooth: Keysym = Keysym(0x1008ff94);
    pub const XF86WLAN: Keysym = Keysym(0x1008ff95);
    pub const XF86UWB: Keysym = Keysym(0x1008ff96);
    pub const XF86AudioForward: Keysym = Keysym(0x1008ff97);
    pub const XF86AudioRepeat: Keysym = Keysym(0x1008ff98);
    pub const XF86AudioRandomPlay: Keysym = Keysym(0x1008ff99);
    pub const XF86Subtitle: Keysym = Keysym(0x1008ff9a);
    pub const XF86AudioCycleTrack: Keysym = Keysym(0x1008ff9b);
    pub const XF86CycleAngle: Keysym = Keysym(0x1008ff9c);
    pub const XF86FrameBack: Keysym = Keysym(0x1008ff9d);
    pub const XF86FrameForward: Keysym = Keysym(0x1008ff9e);
    pub const XF86Time: Keysym = Keysym(0x1008ff9f);
    pub const XF86Select: Keysym = Keysym(0x1008ffa0);
    pub const XF86View: Keysym = Keysym(0x1008ffa1);
    pub const XF86TopMenu: Keysym = Keysym(0x1008ffa2);
    pub const XF86Red: Keysym = Keysym(0x1008ffa3);
    pub const XF86Green: Keysym = Keysym(0x1008ffa4);
    pub const XF86Yellow: Keysym = Keysym(0x1008ffa5);
    pub const XF86Blue: Keysym = Keysym(0x1008ffa6);
    pub const XF86Suspend: Keysym = Keysym(0x1008ffa7);
    pub const XF86Hibernate: Keysym = Keysym(0x1008ffa8);
    pub const XF86TouchpadToggle: Keysym = Keysym(0x1008ffa9);
    pub const XF86TouchpadOn: Keysym = Keysym(0x1008ffb0);
    pub const XF86TouchpadOff: Keysym = Keysym(0x1008ffb1);
    pub const XF86AudioMicMute: Keysym = Keysym(0x1008ffb2);
    pub const XF86Keyboard: Keysym = Keysym(0x1008ffb3);
    pub const XF86WWAN: Keysym = Keysym(0x1008ffb4);
    pub const XF86RFKill: Keysym = Keysym(0x1008ffb5);
    pub const XF86AudioPreset: Keysym = Keysym(0x1008ffb6);
    pub const XF86RotationLockToggle: Keysym = Keysym(0x1008ffb7);
    pub const XF86FullScreen: Keysym = Keysym(0x1008ffb8);
}
