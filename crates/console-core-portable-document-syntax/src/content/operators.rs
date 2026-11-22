#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the PDF operator table defines the operand types and typed dispatch together"
  )
)]
macro_rules! pdf_operators {
  (
    $($spelling:literal => $name:ident $(<$($lifetime:lifetime),+>)?,
      $kind:ident $(($($index:literal : $field:ty),+))?;)*
  ) => {
    $(pdf_operators!(@define $kind, $spelling, $name $(<$($lifetime),+>)? $(( $($index: $field),+ ))?);)*

    #[derive(Debug, PartialEq, Clone)]
    pub enum TypedInstruction<'b, 'a> {
      $($name($name $(<$($lifetime),+>)?),)*
      Fallback(&'b Operator<'a>),
    }

    impl<'b, 'a> TypedInstruction<'b, 'a> {
      #[inline(always)]
      pub(crate) fn dispatch(
        instruction: &Instruction<'b, 'a>,
      ) -> Result<Option<Self>, console_core_never::Never> {
        Ok(match instruction.operator.as_ref() {
          $($spelling => $name::from_stack(instruction.operands).map(Self::$name),)*
          _ => Some(Self::Fallback(instruction.operator)),
        })
      }
    }
  };
  (@define unit, $spelling:literal, $name:ident) => {
    #[derive(Debug, PartialEq, Clone)]
    pub struct $name;

    op0!($name, const { operator_name($spelling) });
  };
  (@define all, $spelling:literal, $name:ident) => {
    pdf_operators!(@operands $name(Vec<Number>));
    op_all!($name, const { operator_name($spelling) });
  };
  (
    @define fixed, $spelling:literal, $name:ident
    $(<$($lifetime:lifetime),+>)? ($($index:literal : $field:ty),+)
  ) => {
    pdf_operators!(@operands $name $(<$($lifetime),+>)? ($($field),+));
    op_fixed!($name $(<$($lifetime),+>)?, const { operator_name($spelling) }, $($index),+);
  };
  (@define manual, $spelling:literal, $name:ident <$($lifetime:lifetime),+>) => {};
  (@operands $name:ident $(<$($lifetime:lifetime),+>)? ($($field:ty),+)) => {
    #[derive(Debug, PartialEq, Clone)]
    #[cfg_attr(
      dylint_lib = "explicit065_checked_constructors",
      expect(
        explicit065_checked_constructors,
        reason = "Each field is an independent PDF operand and TryFrom only checks the instruction variant"
      )
    )]
    pub struct $name $(<$($lifetime),+>)? ($(pub $field),+);
  };
}

const fn operator_name(spelling: &'static [u8]) -> &'static str {
  match core::str::from_utf8(spelling) {
    Ok(name) => name,
    Err(_) => panic!("a PDF operator must have an ASCII spelling"),
  }
}

pdf_operators! {
  // Compatibility operators.
  b"BX" => BeginCompatibility, unit;
  b"EX" => EndCompatibility, unit;

  // Graphic state operators.
  b"q" => SaveState, unit;
  b"Q" => RestoreState, unit;
  b"cm" => Transform, fixed(0: Number, 1: Number, 2: Number, 3: Number, 4: Number, 5: Number);
  b"w" => LineWidth, fixed(0: Number);
  b"J" => LineCap, fixed(0: Number);
  b"j" => LineJoin, fixed(0: Number);
  b"M" => MiterLimit, fixed(0: Number);
  b"d" => DashPattern<'b, 'a>, fixed(0: &'b Array<'a>, 1: Number);
  b"ri" => RenderingIntent<'b, 'a>, fixed(0: &'b Name<'a>);
  b"i" => FlatnessTolerance, fixed(0: Number);
  b"gs" => SetGraphicsState<'b, 'a>, fixed(0: &'b Name<'a>);

  // Path-construction operators.
  b"m" => MoveTo, fixed(0: Number, 1: Number);
  b"l" => LineTo, fixed(0: Number, 1: Number);
  b"c" => CubicTo, fixed(0: Number, 1: Number, 2: Number, 3: Number, 4: Number, 5: Number);
  b"v" => CubicStartTo, fixed(0: Number, 1: Number, 2: Number, 3: Number);
  b"y" => CubicEndTo, fixed(0: Number, 1: Number, 2: Number, 3: Number);
  b"h" => ClosePath, unit;
  b"re" => RectPath, fixed(0: Number, 1: Number, 2: Number, 3: Number);

  // Path-painting operators.
  b"S" => StrokePath, unit;
  b"s" => CloseAndStrokePath, unit;
  b"f" => FillPathNonZero, unit;
  b"F" => FillPathNonZeroCompatibility, unit;
  b"f*" => FillPathEvenOdd, unit;
  b"B" => FillAndStrokeNonZero, unit;
  b"B*" => FillAndStrokeEvenOdd, unit;
  b"b" => CloseFillAndStrokeNonZero, unit;
  b"b*" => CloseFillAndStrokeEvenOdd, unit;
  b"n" => EndPath, unit;

  // Clipping path operators.
  b"W" => ClipNonZero, unit;
  b"W*" => ClipEvenOdd, unit;

  // Colour operators.
  b"CS" => ColorSpaceStroke<'b, 'a>, fixed(0: &'b Name<'a>);
  b"cs" => ColorSpaceNonStroke<'b, 'a>, fixed(0: &'b Name<'a>);
  b"SC" => StrokeColor, all;
  b"SCN" => StrokeColorNamed<'b, 'a>, manual;
  b"sc" => NonStrokeColor, all;
  b"scn" => NonStrokeColorNamed<'b, 'a>, manual;
  b"G" => StrokeColorDeviceGray, fixed(0: Number);
  b"g" => NonStrokeColorDeviceGray, fixed(0: Number);
  b"RG" => StrokeColorDeviceRgb, fixed(0: Number, 1: Number, 2: Number);
  b"rg" => NonStrokeColorDeviceRgb, fixed(0: Number, 1: Number, 2: Number);
  b"K" => StrokeColorCmyk, fixed(0: Number, 1: Number, 2: Number, 3: Number);
  b"k" => NonStrokeColorCmyk, fixed(0: Number, 1: Number, 2: Number, 3: Number);

  // Shading operator.
  b"sh" => Shading<'b, 'a>, fixed(0: &'b Name<'a>);

  // XObject operator.
  b"Do" => XObject<'b, 'a>, fixed(0: &'b Name<'a>);

  // Inline-image operators.
  b"BI" => InlineImage<'b, 'a>, fixed(0: &'b Stream<'a>);

  // Text-state operators.
  b"Tc" => CharacterSpacing, fixed(0: Number);
  b"Tw" => WordSpacing, fixed(0: Number);
  b"Tz" => HorizontalScaling, fixed(0: Number);
  b"TL" => TextLeading, fixed(0: Number);
  b"Tf" => TextFont<'b, 'a>, fixed(0: &'b Name<'a>, 1: Number);
  b"Tr" => TextRenderingMode, fixed(0: Number);
  b"Ts" => TextRise, fixed(0: Number);

  // Text-object operators.
  b"BT" => BeginText, unit;
  b"ET" => EndText, unit;

  // Text-positioning operators.
  b"Td" => NextLine, fixed(0: Number, 1: Number);
  b"TD" => NextLineAndSetLeading, fixed(0: Number, 1: Number);
  b"Tm" => SetTextMatrix, fixed(0: Number, 1: Number, 2: Number, 3: Number, 4: Number, 5: Number);
  b"T*" => NextLineUsingLeading, unit;

  // Text-showing operators.
  b"Tj" => ShowText<'b, 'a>, fixed(0: &'b object::String<'a>);
  b"'" => NextLineAndShowText<'b, 'a>, fixed(0: &'b object::String<'a>);
  b"\"" => ShowTextWithParameters<'b, 'a>, fixed(0: Number, 1: Number, 2: &'b object::String<'a>);
  b"TJ" => ShowTexts<'b, 'a>, fixed(0: &'b Array<'a>);

  // Type 3 font operators.
  b"d0" => ColorGlyph, fixed(0: Number, 1: Number);
  b"d1" => ShapeGlyph, fixed(0: Number, 1: Number, 2: Number, 3: Number, 4: Number, 5: Number);

  // Marked content operators.
  b"MP" => MarkedContentPoint<'b, 'a>, fixed(0: &'b Name<'a>);
  b"DP" => MarkedContentPointWithProperties<'b, 'a>, fixed(0: &'b Name<'a>, 1: &'b Object<'a>);
  b"BMC" => BeginMarkedContent<'b, 'a>, fixed(0: &'b Name<'a>);
  b"BDC" => BeginMarkedContentWithProperties<'b, 'a>, fixed(0: &'b Name<'a>, 1: &'b Object<'a>);
  b"EMC" => EndMarkedContent, unit;
}
