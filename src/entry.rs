#[derive(Debug, Clone)]
pub struct Entry {
    pub slug: String,
    pub value: String,
    pub childs: Vec<(String, String)>,
}

impl Entry {
    pub fn new(slug: String, value: String) -> Self {
        Self {
            slug,
            value,
            childs: Vec::new(),
        }
    }

    pub fn add_child(&mut self, slug: String, value: String) {
        self.childs.push((slug, value));
    }

    pub fn print(&self) {
        println!("{} = {}", self.slug, self.value);
        for (slug, value) in self.childs.iter() {
            println!("-- {} = {}", slug, value);
        }
        println!("--------------\n\n");
    }

    pub fn get_value_from_slug(&self, slug: &str) -> Option<String> {
        let _format = |v: &str| -> String {
            let v = v.replace("{\"{\"}", "{").replace("{\"}\"}", "}");
            let v = v.trim();
            if !v.contains("\"") {
                format!("\"{}\"", v)
            } else {
                format!("\"{}\"", v.replace("\"", "\\\""))
            }
        };
        if slug == self.slug {
            return Some(_format(&self.value));
        } else {
            // remove the first part split with "_"
            let parts = slug.split("_").skip(1).collect::<Vec<_>>();
            let new_slug = parts.join("_");
            //eprintln!("new_slug: {:?}", new_slug);
            for (k, v) in self.childs.iter() {
                if k == slug
                    || k == &format!(".{}", slug)
                    || k == &new_slug
                    || k == &format!(".{}", new_slug)
                {
                    return Some(_format(&v));
                }
            }
        }

        return None;
    }
}
