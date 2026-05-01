pub const DEFAULT_PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, Copy)]
pub struct PageRequest {
    pub page: usize, // 1-indexed
    pub page_size: usize,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

pub struct PageOutcome<T> {
    pub items: Vec<T>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub total_pages: usize,
}

pub fn paginate<T: Clone>(all: &[T], req: PageRequest) -> PageOutcome<T> {
    let page_size = req.page_size.max(1);
    let total = all.len();
    let total_pages = total.div_ceil(page_size).max(1);
    let page = req.page.max(1).min(total_pages);
    let start = (page - 1) * page_size;
    let end = (start + page_size).min(total);
    PageOutcome {
        items: all[start..end].to_vec(),
        page,
        page_size,
        total,
        total_pages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_page_default_size() {
        let xs: Vec<i32> = (0..120).collect();
        let out = paginate(&xs, PageRequest::default());
        assert_eq!(out.items.len(), 50);
        assert_eq!(out.items[0], 0);
        assert_eq!(out.total, 120);
        assert_eq!(out.total_pages, 3);
        assert_eq!(out.page, 1);
    }

    #[test]
    fn last_page_partial() {
        let xs: Vec<i32> = (0..120).collect();
        let out = paginate(
            &xs,
            PageRequest {
                page: 3,
                page_size: 50,
            },
        );
        assert_eq!(out.items.len(), 20);
        assert_eq!(out.items[0], 100);
    }

    #[test]
    fn empty_input_yields_one_empty_page() {
        let xs: Vec<i32> = vec![];
        let out = paginate(&xs, PageRequest::default());
        assert!(out.items.is_empty());
        assert_eq!(out.total_pages, 1);
        assert_eq!(out.page, 1);
    }
}
