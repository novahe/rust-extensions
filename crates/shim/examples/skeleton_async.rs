/*
   Copyright The containerd Authors.

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
*/

use std::sync::Arc;

use async_trait::async_trait;
use containerd_shim::{
    asynchronous::{run, spawn, ExitSignal, Shim},
    publisher::RemotePublisher,
    Config, Error, StartOpts, TtrpcResult,
};
#[cfg(feature = "sandbox")]
use containerd_shim_protos::sandbox::sandbox_ttrpc::Sandbox;
use containerd_shim_protos::{
    api, api::DeleteResponse, shim_async::Task, ttrpc::r#async::TtrpcContext,
};
use log::info;

#[derive(Clone)]
struct Service {
    exit: Arc<ExitSignal>,
}

#[async_trait]
impl Shim for Service {
    type T = Service;

    #[cfg(feature = "sandbox")]
    type S = Service;

    async fn new(_runtime_id: &str, _id: &str, _namespace: &str, _config: &mut Config) -> Self {
        Service {
            exit: Arc::new(ExitSignal::default()),
        }
    }

    async fn start_shim(&mut self, opts: StartOpts) -> Result<String, Error> {
        let grouping = opts.id.clone();
        let address = spawn(opts, &grouping, Vec::new()).await?;
        Ok(address)
    }

    async fn delete_shim(&mut self) -> Result<DeleteResponse, Error> {
        Ok(DeleteResponse::new())
    }

    async fn wait(&mut self) {
        self.exit.wait().await;
    }

    async fn create_task_service(&self, _publisher: RemotePublisher) -> Self::T {
        self.clone()
    }

    #[cfg(feature = "sandbox")]
    async fn create_sandbox_service(&self) -> Self::S {
        self.clone()
    }
}

#[cfg(feature = "sandbox")]
#[async_trait]
impl Sandbox for Service {
    async fn create_sandbox(
        &self,
        _ctx: &TtrpcContext,
        _req: containerd_shim_protos::sandbox::sandbox::CreateSandboxRequest,
    ) -> TtrpcResult<containerd_shim_protos::sandbox::sandbox::CreateSandboxResponse> {
        Ok(containerd_shim_protos::sandbox::sandbox::CreateSandboxResponse::default())
    }

    async fn start_sandbox(
        &self,
        _ctx: &TtrpcContext,
        _req: containerd_shim_protos::sandbox::sandbox::StartSandboxRequest,
    ) -> TtrpcResult<containerd_shim_protos::sandbox::sandbox::StartSandboxResponse> {
        Ok(containerd_shim_protos::sandbox::sandbox::StartSandboxResponse::default())
    }

    async fn platform(
        &self,
        _ctx: &TtrpcContext,
        _req: containerd_shim_protos::sandbox::sandbox::PlatformRequest,
    ) -> TtrpcResult<containerd_shim_protos::sandbox::sandbox::PlatformResponse> {
        Ok(containerd_shim_protos::sandbox::sandbox::PlatformResponse::default())
    }

    async fn stop_sandbox(
        &self,
        _ctx: &TtrpcContext,
        _req: containerd_shim_protos::sandbox::sandbox::StopSandboxRequest,
    ) -> TtrpcResult<containerd_shim_protos::sandbox::sandbox::StopSandboxResponse> {
        Ok(containerd_shim_protos::sandbox::sandbox::StopSandboxResponse::default())
    }

    async fn wait_sandbox(
        &self,
        _ctx: &TtrpcContext,
        _req: containerd_shim_protos::sandbox::sandbox::WaitSandboxRequest,
    ) -> TtrpcResult<containerd_shim_protos::sandbox::sandbox::WaitSandboxResponse> {
        Ok(containerd_shim_protos::sandbox::sandbox::WaitSandboxResponse::default())
    }

    async fn sandbox_status(
        &self,
        _ctx: &TtrpcContext,
        _req: containerd_shim_protos::sandbox::sandbox::SandboxStatusRequest,
    ) -> TtrpcResult<containerd_shim_protos::sandbox::sandbox::SandboxStatusResponse> {
        Ok(containerd_shim_protos::sandbox::sandbox::SandboxStatusResponse::default())
    }

    async fn ping_sandbox(
        &self,
        _ctx: &TtrpcContext,
        _req: containerd_shim_protos::sandbox::sandbox::PingRequest,
    ) -> TtrpcResult<containerd_shim_protos::sandbox::sandbox::PingResponse> {
        Ok(containerd_shim_protos::sandbox::sandbox::PingResponse::default())
    }

    async fn shutdown_sandbox(
        &self,
        _ctx: &TtrpcContext,
        _req: containerd_shim_protos::sandbox::sandbox::ShutdownSandboxRequest,
    ) -> TtrpcResult<containerd_shim_protos::sandbox::sandbox::ShutdownSandboxResponse> {
        Ok(containerd_shim_protos::sandbox::sandbox::ShutdownSandboxResponse::default())
    }
}

#[async_trait]
impl Task for Service {
    async fn connect(
        &self,
        _ctx: &TtrpcContext,
        _req: api::ConnectRequest,
    ) -> TtrpcResult<api::ConnectResponse> {
        info!("Connect request");
        Ok(api::ConnectResponse {
            version: String::from("example"),
            ..Default::default()
        })
    }

    async fn shutdown(
        &self,
        _ctx: &TtrpcContext,
        _req: api::ShutdownRequest,
    ) -> TtrpcResult<api::Empty> {
        info!("Shutdown request");
        self.exit.signal();
        Ok(api::Empty::default())
    }
}

#[tokio::main]
async fn main() {
    run::<Service>("io.containerd.empty.v1", None).await;
}
