#[derive(Debug)]
pub struct Pdf {
    document: poppler::Document,
    page_index: i32,
}

impl Pdf {
    pub fn new(document: poppler::Document) -> Self {
        Pdf {
            document,
            page_index: 0,
        }
    }

    pub fn has_previous_page(&self) -> bool {
        self.page_index > 0
    }

    pub fn has_next_page(&self) -> bool {
        self.page_index < self.document.n_pages() - 1
    }

    pub fn current_page(&self) -> Option<poppler::Page> {
        self.document.page(self.page_index)
    }

    pub fn update_page(&mut self, change: PdfPageChange) {
        match change {
            PdfPageChange::Previous if self.has_previous_page() => self.page_index -= 1,
            PdfPageChange::Next if self.has_next_page() => self.page_index += 1,
            _ => (),
        }
    }
}

#[derive(Debug)]
pub enum PdfPageChange {
    Previous,
    Next,
}
