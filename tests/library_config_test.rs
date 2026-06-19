#[cfg(test)]
mod tests {
    use mellow::init_channels;
    use std::sync::mpsc;
    use tokio::sync::mpsc as tokio_mpsc;

    use mellow::library::{LibraryConfig, LibraryRequest};
    use mellow::ui::UpdateUI;

    struct ConfigTester {
        config: LibraryConfig,
        _ui_rx: tokio_mpsc::UnboundedReceiver<UpdateUI>,
        _library_rx: mpsc::Receiver<LibraryRequest>,
    }

    #[test]
    fn library_config_correctness() {
        mellow::init_globals();
        let mut config_tester = ConfigTester::default();
        config_tester.test_empty_by_default();
        config_tester.test_add_library();
        config_tester.test_set_libraries();
        config_tester.test_remove_library();
        config_tester.test_sort_alphabetically();
        config_tester.test_reject_duplicates();
        config_tester.test_reject_empty();
    }

    impl ConfigTester {
        fn test_empty_by_default(&self) {
            assert!(&self.config.directories_string().is_empty());
        }

        fn test_add_library(&mut self) {
            self.config.add_library("/test".into());
            assert_eq!(
                self.config.directories_string(),
                &["/test"],
                "`test_add_library()`"
            );
        }

        fn test_set_libraries(&mut self) {
            self.config.set_libraries(vec![
                "/some/directory".into(),
                "/some/folder".into(),
                "/some/other/directory".into(),
            ]);
            assert_eq!(
                self.config.directories_string(),
                &["/some/directory", "/some/folder", "/some/other/directory",],
                "`test_set_libraries()`"
            );
        }

        fn test_remove_library(&mut self) {
            self.config.remove_library(1);
            assert_eq!(
                self.config.directories_string(),
                &["/some/directory", "/some/other/directory"],
                "`test_remove_library()`"
            );
        }

        fn test_sort_alphabetically(&mut self) {
            self.config.add_library("/audio".into());
            assert_eq!(
                self.config.directories_string(),
                &["/audio", "/some/directory", "/some/other/directory",],
                "`test_sort_alphabetically()`"
            );
        }

        fn test_reject_duplicates(&mut self) {
            self.config.add_library("/some/directory".into());
            assert_eq!(
                self.config.directories_string(),
                &["/audio", "/some/directory", "/some/other/directory",],
                "`test_reject_duplicates()`"
            );
        }

        fn test_reject_empty(&mut self) {
            self.config.add_library("".into());
            assert_eq!(
                self.config.directories_string(),
                &["/audio", "/some/directory", "/some/other/directory",],
                "`test_reject_empty()`"
            );
        }
    }

    impl Default for ConfigTester {
        fn default() -> Self {
            let (_, library_rx, ui_rx, _) = init_channels().unwrap();
            ConfigTester {
                config: LibraryConfig::new(vec![]),
                _ui_rx: ui_rx,
                _library_rx: library_rx,
            }
        }
    }
}
