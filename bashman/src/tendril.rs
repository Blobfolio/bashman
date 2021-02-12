use tendril::StrTendril;



/// Join.
pub(super) fn join(src: &[StrTendril], glue: &StrTendril) -> StrTendril {
	let len: usize = src.len();
	let mut idx: usize = 0;
	src.iter()
		.fold(StrTendril::new(), |mut a, b| {
			idx += 1;
			a.push_tendril(b);
			if idx < len {
				a.push_tendril(glue);
			}
			a
		})
}

/// Trim.
pub(super) fn trim(txt: &mut StrTendril) {
	trim_start(txt);
	trim_end(txt);
}

/// Trim Start.
pub(super) fn trim_start(txt: &mut StrTendril) {
	let len: u32 = txt.as_bytes()
		.iter()
		.take_while(|c| matches!(*c, b'\t' | b'\n' | b'\x0C' | b'\r' | b' '))
		.count() as u32;
	if 0 != len {
		txt.pop_front(len);
	}
}

/// Trim End.
pub(super) fn trim_end(txt: &mut StrTendril) {
	let len: u32 = txt.as_bytes()
		.iter()
		.rev()
		.take_while(|c| matches!(*c, b'\t' | b'\n' | b'\x0C' | b'\r' | b' '))
		.count() as u32;
	if 0 != len {
		txt.pop_back(len);
	}
}
