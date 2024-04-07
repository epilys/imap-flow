use bytes::BytesMut;
use imap_flow::{
    client::{ClientFlow, ClientFlowAction, ClientFlowEvent, ClientFlowOptions},
    stream::AnyStream,
};
use imap_types::{
    command::{Command, CommandBody},
    core::Tag,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{tcp::WriteHalf, TcpStream},
    select,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:12345").await.unwrap();
    let mut buffers = Buffers::default();

    let mut client = ClientFlow::new(ClientFlowOptions::default());

    let greeting = 'outer: loop {
        buffers.progress(&mut stream).await;
        loop {
            match client.progress().unwrap() {
                ClientFlowAction::ReadBytes => {
                    client.read(&mut buffers.read_buffer);
                    break;
                }
                ClientFlowAction::WriteBytes => {
                    client.write(&mut buffers.write_buffer);
                    break;
                }
                ClientFlowAction::HandleEvent(ClientFlowEvent::GreetingReceived { greeting }) => {
                    break 'outer greeting;
                }
                action => {
                    println!("unexpected action: {action:?}");
                }
            }
        }
    };

    println!("received greeting: {greeting:?}");

    let handle = client.enqueue_command(Command {
        tag: Tag::try_from("A1").unwrap(),
        body: CommandBody::login("Al¹cE", "pa²²w0rd").unwrap(),
    });

    loop {
        buffers.progress(&mut stream).await;
        loop {
            match client.progress().unwrap() {
                ClientFlowAction::ReadBytes => {
                    client.read(&mut buffers.read_buffer);
                    break;
                }
                ClientFlowAction::WriteBytes => {
                    client.write(&mut buffers.write_buffer);
                    break;
                }
                ClientFlowAction::HandleEvent(ClientFlowEvent::CommandSent {
                    handle: got_handle,
                    command,
                }) => {
                    println!("command sent: {got_handle:?}, {command:?}");
                    assert_eq!(handle, got_handle);
                }
                ClientFlowAction::HandleEvent(ClientFlowEvent::CommandRejected {
                    handle: got_handle,
                    command,
                    status,
                }) => {
                    println!("command rejected: {got_handle:?}, {command:?}, {status:?}");
                    assert_eq!(handle, got_handle);
                }
                ClientFlowAction::HandleEvent(ClientFlowEvent::DataReceived { data }) => {
                    println!("data received: {data:?}");
                }
                ClientFlowAction::HandleEvent(ClientFlowEvent::StatusReceived { status }) => {
                    println!("status received: {status:?}");
                }
                ClientFlowAction::HandleEvent(ClientFlowEvent::ContinuationRequestReceived {
                    continuation_request,
                }) => {
                    println!("unexpected continuation request received: {continuation_request:?}");
                }
                ClientFlowAction::HandleEvent(event) => {
                    println!("{event:?}");
                }
            }
        }
    }
}

#[derive(Default)]
struct Buffers {
    read_buffer: BytesMut,
    write_buffer: BytesMut,
}

impl Buffers {
    async fn progress(&mut self, stream: &mut TcpStream) {
        // if !self.write_buffer.is_empty() {
        //     stream.write_buf(&mut self.write_buffer).await.unwrap();
        // } else {
        //     stream.read_buf(&mut self.read_buffer).await.unwrap();
        // }
        let (mut read_stream, mut write_stream) = stream.split();
        select! {
            _ = write_if_not_empty(&mut write_stream, &mut self.write_buffer) => (),
            _ = read_stream.read_buf(&mut self.read_buffer) => (),
        }
    }
}

async fn write_if_not_empty(write_stream: &mut WriteHalf<'_>, write_buffer: &mut BytesMut) {
    if write_buffer.is_empty() {
        std::future::pending().await
    } else {
        write_stream.write_buf(write_buffer).await.unwrap();
    }
}
