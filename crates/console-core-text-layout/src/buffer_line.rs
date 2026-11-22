use core::mem;

use crate::{
  Align, Attrs, AttrsList, Cached, Ellipsize, FontSystem, Hinting, LayoutLine, LayoutRunIter,
  LineEnding, ShapeLine, Shaping, Wrap,
};

#[derive(Clone, Debug)]
pub struct BufferLine {
  text: String,
  ending: LineEnding,
  attrs_list: AttrsList,
  align: Option<Align>,
  shape_opt: Cached<ShapeLine>,
  layout_opt: Cached<Vec<LayoutLine>>,
  shaping: Shaping,
  metadata: Option<u32>,
}

impl BufferLine {
  pub fn new<T: Into<String>>(
    text: T,
    ending: LineEnding,
    attrs_list: AttrsList,
    shaping: Shaping,
  ) -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        text: <T as Into<String>>::into(text),
        ending,
        attrs_list,
        align: None,
        shape_opt: Cached::default(),
        layout_opt: Cached::default(),
        shaping,
        metadata: None,
      })
    }
  }

  pub fn reset_new<T: Into<String>>(
    &mut self,
    text: T,
    ending: LineEnding,
    attrs_list: AttrsList,
    shaping: Shaping,
  ) -> Result<(), console_core_never::Never> {
    {
      self.text = <T as Into<String>>::into(text);
      self.ending = ending;
      self.attrs_list = attrs_list;
      self.align = None;

      let Ok(()) = self.shape_opt.set_unused();

      let Ok(()) = self.layout_opt.set_unused();

      self.shaping = shaping;
      self.metadata = None;

      Ok(())
    }
  }

  pub fn text(&self) -> Result<&str, console_core_never::Never> {
    Ok(&self.text)
  }

  pub fn set_text<T: AsRef<str>>(
    &mut self,
    text: T,
    ending: LineEnding,
    attrs_list: AttrsList,
  ) -> Result<crate::Modification, console_core_never::Never> {
    {
      let text_2 = text.as_ref();

      Ok(
        match ::core::primitive::bool::then_some(text_2 != self.text || ending != self.ending || attrs_list != self.attrs_list, ()) {
          ::core::option::Option::Some(()) => {
            self.text.clear();
            self.text.push_str(text_2);
            self.ending = ending;
            self.attrs_list = attrs_list;

            let Ok(()) = self.reset();

            crate::Modification::Changed
          }
          ::core::option::Option::None => crate::Modification::Unchanged,
        },
      )
    }
  }

  pub fn into_text(self) -> Result<String, console_core_never::Never> {
    Ok(self.text)
  }

  pub const fn ending(&self) -> Result<LineEnding, console_core_never::Never> {
    Ok(self.ending)
  }

  pub fn set_ending(
    &mut self,
    ending: LineEnding,
  ) -> Result<crate::Modification, console_core_never::Never> {
    {
      Ok(match ::core::primitive::bool::then_some(ending != self.ending, ()) {
        ::core::option::Option::Some(()) => {
          self.ending = ending;

          let Ok(()) = self.reset_shaping();

          crate::Modification::Changed
        }
        ::core::option::Option::None => crate::Modification::Unchanged,
      })
    }
  }

  pub const fn attrs_list(&self) -> Result<&AttrsList, console_core_never::Never> {
    Ok(&self.attrs_list)
  }

  pub fn set_attrs_list(
    &mut self,
    attrs_list: AttrsList,
  ) -> Result<crate::Modification, console_core_never::Never> {
    {
      Ok(match ::core::primitive::bool::then_some(attrs_list != self.attrs_list, ()) {
        ::core::option::Option::Some(()) => {
          self.attrs_list = attrs_list;

          let Ok(()) = self.reset_shaping();

          crate::Modification::Changed
        }
        ::core::option::Option::None => crate::Modification::Unchanged,
      })
    }
  }

  pub const fn align(&self) -> Result<Option<Align>, console_core_never::Never> {
    Ok(self.align)
  }

  pub fn set_align(
    &mut self,
    align: Option<Align>,
  ) -> Result<crate::Modification, console_core_never::Never> {
    {
      Ok(match ::core::primitive::bool::then_some(align != self.align, ()) {
        ::core::option::Option::Some(()) => {
          self.align = align;

          let Ok(()) = self.reset_layout();

          crate::Modification::Changed
        }
        ::core::option::Option::None => crate::Modification::Unchanged,
      })
    }
  }

  pub fn append(&mut self, other: &Self) -> Result<(), console_core_never::Never> {
    {
      let Ok(len) = console_core_number_conversion::fitted::<_, u32>(self.text.len());
      let Ok(text) = other.text();
      self.text.push_str(text);

      let Ok(ending) = other.ending();
      self.ending = ending;

      let Ok(defaults) = other.attrs_list.defaults();
      let Ok(own_defaults) = self.attrs_list.defaults();

      match ::core::primitive::bool::then_some(defaults != own_defaults, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(()) = self.attrs_list.add_span(
            (match console_core_number_conversion::fitted::<_, u32>(len) {
              Ok(value) => value,
            })
              ..u32::saturating_add(
                match console_core_number_conversion::fitted::<_, u32>(len) {
                  Ok(value) => value,
                },
                match console_core_number_conversion::fitted::<_, u32>(
                  match other.text() {
                    Ok(value) => value,
                  }
                  .len(),
                ) {
                  Ok(value) => value,
                },
              ),
            &match other.attrs_list.defaults() {
              Ok(value) => value,
            },
          );
        }
        ::core::option::Option::None => {}
      }

      let Ok(spans_iter) = other.attrs_list.spans_iter();

      for (other_range, attrs) in spans_iter {
        let range = u32::saturating_add(
          other_range.start,
          match console_core_number_conversion::fitted::<_, u32>(len) {
            Ok(value) => value,
          },
        )
          ..u32::saturating_add(
            other_range.end,
            match console_core_number_conversion::fitted::<_, u32>(len) {
              Ok(value) => value,
            },
          );

        let Ok(attrs_attrs) = attrs.as_attrs();

        let Ok(()) = self.attrs_list.add_span(range, &attrs_attrs);
      }

      let Ok(()) = self.reset();

      Ok(())
    }
  }

  pub fn split_off(&mut self, index: u32) -> Result<Self, console_core_never::Never> {
    {
      let text = self
        .text
        .split_off(match console_core_number_conversion::index(index) {
          Ok(position) => position,
        });
      let Ok(attrs_list) = self.attrs_list.split_off(index);

      let Ok(()) = self.reset();

      let Ok(mut new) = Self::new(text, self.ending, attrs_list, self.shaping);

      self.ending = LineEnding::None;
      new.align = self.align;
      Ok(new)
    }
  }

  pub fn reset(&mut self) -> Result<(), console_core_never::Never> {
    {
      self.metadata = None;

      let Ok(()) = self.reset_shaping();

      Ok(())
    }
  }

  pub fn reset_shaping(&mut self) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.shape_opt.set_unused();

      let Ok(()) = self.reset_layout();

      Ok(())
    }
  }

  pub fn reset_layout(&mut self) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.layout_opt.set_unused();

      Ok(())
    }
  }

  pub fn shape(
    &mut self,
    font_system: &mut FontSystem,
    tab_width: u16,
  ) -> Result<&ShapeLine, console_core_never::Never> {
    {
      match self.shape_opt {
        Cached::Unused(_) => {
          let Ok(take_unused) = self.shape_opt.take_unused();
          let mut line = match take_unused {
            Some(present) => present,
            None => {
              let Ok(empty) = ShapeLine::empty();

              empty
            }
          };

          let Ok(()) = line.build(
            font_system,
            &self.text,
            &self.attrs_list,
            self.shaping,
            tab_width,
          );

          let Ok(()) = self.shape_opt.set_used(line);

          let Ok(()) = self.layout_opt.set_unused();
        }
        Cached::Used(_) => {}
      }

      Ok(match self.shape_opt.value() {
        Ok(value) => value,
      })
    }
  }

  pub const fn shape_opt(&self) -> Result<Option<&ShapeLine>, console_core_never::Never> {
    {
      let Ok(shape_opt) = self.shape_opt.get();

      Ok(shape_opt)
    }
  }

  pub const fn needs_reshaping(&self) -> Result<crate::Reshaping, console_core_never::Never> {
    {
      Ok(match (&self.shape_opt, &self.layout_opt) {
        (Cached::Used(_), Cached::Used(_)) => crate::Reshaping::Current,
        (Cached::Unused(_), Cached::Used(_))
        | (Cached::Used(_), Cached::Unused(_))
        | (Cached::Unused(_), Cached::Unused(_)) => crate::Reshaping::Needed,
      })
    }
  }

  pub fn layout(
    &mut self,
    font_system: &mut FontSystem,
    options: BufferLineLayout,
  ) -> Result<&[LayoutLine], console_core_never::Never> {
    let BufferLineLayout {
      font_size,
      width_opt,
      wrap,
      ellipsize,
      match_mono_width,
      tab_width,
      hinting,
    } = options;

    {
      match self.layout_opt {
        Cached::Unused(_) => {
          let align = self.align;
          let Ok(take_unused) = self.layout_opt.take_unused();
          let mut layout = match take_unused {
            Some(present) => present,
            None => Vec::with_capacity(1),
          };
          let Ok(shape) = self.shape(font_system, tab_width);

          let Ok(()) = shape.layout_to_buffer(
            &mut font_system.shape_buffer,
            crate::BufferLayout {
              font_size,
              width_opt,
              wrap,
              ellipsize,
              align,
              match_mono_width,
              hinting,
            },
            &mut layout,
          );

          let Ok(()) = self.layout_opt.set_used(layout);
        }
        Cached::Used(_) => {}
      }

      Ok(match self.layout_opt.value() {
        Ok(value) => value,
      })
    }
  }

  pub const fn layout_opt(&self) -> Result<Option<&Vec<LayoutLine>>, console_core_never::Never> {
    {
      let Ok(layout_opt) = self.layout_opt.get();

      Ok(layout_opt)
    }
  }

  pub fn layout_runs(
    &self,
    height_opt: Option<f32>,
    height: impl Into<crate::LineHeight>,
  ) -> Result<LayoutRunIter<'_>, console_core_never::Never> {
    let crate::LineHeight(line_height) = Into::<crate::LineHeight>::into(height);

    {
      let Ok(layout_run_iter) = LayoutRunIter::from_lines(
        core::slice::from_ref(self),
        height_opt,
        line_height,
        crate::ScrollOffset(0.0),
        0,
      );

      Ok(layout_run_iter)
    }
  }

  pub const fn metadata(&self) -> Result<Option<u32>, console_core_never::Never> {
    Ok(self.metadata)
  }

  pub fn set_metadata(&mut self, metadata: u32) -> Result<(), console_core_never::Never> {
    {
      self.metadata = Some(metadata);

      Ok(())
    }
  }

  pub(crate) fn reclaim_attrs(&mut self) -> Result<AttrsList, console_core_never::Never> {
    {
      let Ok(attrs) = Attrs::new();
      let Ok(attrs_list) = AttrsList::new(&attrs);

      Ok(mem::replace(&mut self.attrs_list, attrs_list))
    }
  }

  pub(crate) fn reclaim_text(&mut self) -> Result<String, console_core_never::Never> {
    {
      let mut text = mem::take(&mut self.text);
      text.clear();
      Ok(text)
    }
  }
}

#[derive(Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the public layout request takes a fractional font size in layout pixels"
  )
)]
pub struct BufferLineLayout {
  pub font_size: f32,
  pub width_opt: Option<f32>,
  pub wrap: Wrap,
  pub ellipsize: Ellipsize,
  pub match_mono_width: Option<f32>,
  pub tab_width: u16,
  pub hinting: Hinting,
}
