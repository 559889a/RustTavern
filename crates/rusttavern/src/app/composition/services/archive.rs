use std::sync::Arc;

use crate::app::context::AppContext;
use crate::infrastructure::persistence::data_archive_adapters::{
    DataDirectoryDataRootInitializer, FilesystemDataArchiveFileGateway,
};
use tt_adapter_archive::FileDataArchiveExecutor;
use tt_application::services::data_archive_service::{DataArchiveJobRegistry, DataArchiveService};
use tt_ports::sync::DataChangeReconciler;

pub(super) fn build(
    app_context: &Arc<AppContext>,
    data_change_reconciler: Arc<dyn DataChangeReconciler>,
) -> Arc<DataArchiveService> {
    Arc::new(DataArchiveService::new(
        Arc::new(DataArchiveJobRegistry::new()),
        tokio::runtime::Handle::current(),
        Arc::new(FileDataArchiveExecutor),
        Arc::new(FilesystemDataArchiveFileGateway::new(
            app_context.runtime_paths.clone(),
        )),
        Arc::new(DataDirectoryDataRootInitializer),
        data_change_reconciler,
    ))
}
