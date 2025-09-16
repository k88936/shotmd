//
// Created by kvto on 9/15/24.
//

// You may need to build the project (run Qt uic code generator) to get "ui_shotmd.h" resolved

#include <QScreen>
#include <QMouseEvent>
#include <QPainter>
#include <QBuffer>
#include <QClipboard>
#include <QTimer>
#include <QStandardPaths>
#include <QDateTime>
#include <QGuiApplication>
#include <QScreen>
#include <QMessageBox>
#include "shotmd.h"
#include "ui_shotmd.h"


shotmd::shotmd(QWidget* parent) : QMainWindow(parent), ui(new Ui::shotmd)
{
    setWindowFlags(Qt::WindowStaysOnTopHint | Qt::FramelessWindowHint); // 窗口置顶 + 隐藏标题栏
    QScreen* screen = QGuiApplication::primaryScreen();
    originalPixmap = screen->grabWindow(0);
    screenshotLabel = new QLabel(this);
    screenshotLabel->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    screenshotLabel->setAlignment(Qt::AlignCenter);
    screenshotLabel->setFixedSize(screen->geometry().width(), screen->geometry().height());
    screenshotLabel->setPixmap(originalPixmap);
    screenshotLabel->show();
    ui->setupUi(this);
    state = init;
    //    connect(this,&shotmd::mousePressEvent,this,&shotmd::mousePressEvent);
}

void shotmd::mousePressEvent(QMouseEvent* event)
{
    if (event->button() == Qt::RightButton)
    {
        QGuiApplication::exit();
    }
    switch (state)
    {
    case init:
        anchor = event->pos();
        state = anchorFirst;
        break;
    default: ;
    }
}

void shotmd::mouseMoveEvent(QMouseEvent* event)
{
    switch (state)
    {
    case anchorFirst:
        {
            anchor2 = event->pos();
            QPen pen;
            pen.setColor(Qt::red);
            pen.setWidth(2);
            QPixmap pixmap = originalPixmap.copy();
            QPainter painter(&pixmap);
            painter.setPen(pen);
            painter.drawRect(QRect(anchor, anchor2));
            painter.save();
            screenshotLabel->setPixmap(pixmap);
            break;
        }
    default: ;
    }
}

void shotmd::shot(const QPoint Tl, const QPoint RD)
{
    const QPixmap finalPixmap = originalPixmap.copy(QRect(Tl, RD));
    QByteArray data;
    QBuffer buffer(&data);
    finalPixmap.save(&buffer, "JPEG");
    data = data.toBase64();
    const QString str = QString(R"(<img src="data:image/jpeg;base64,)") + data + QString(R"(" alt="screenshot" />)");
    // Copy to clipboard (both clipboard and selection, though selection is X11-specific)
    QGuiApplication::clipboard()->setText(str, QClipboard::Clipboard);
    QGuiApplication::clipboard()->setText(str, QClipboard::Selection);

    // Save to file: $HOME/Downloads/shotmd-YYYYMMDD-HHMMSS.jpeg
    const QString timestamp = QDateTime::currentDateTime().toString("yyyyMMdd-hhmmss");
    QString downloadsPath = QStandardPaths::writableLocation(QStandardPaths::DownloadLocation);
    if (downloadsPath.isEmpty())
    {
        downloadsPath = QStandardPaths::writableLocation(QStandardPaths::HomeLocation); // Fallback
    }

    const QString filePath = downloadsPath + "/" + QString("shotmd-%1.jpeg").arg(timestamp);
    if (!finalPixmap.save(filePath, "JPEG"))
    {
        // Show error dialog
        QMessageBox::critical(nullptr, "Save Failed",
                              QString("Could not save screenshot to:\n%1\n\nCheck if you have write permissions.").arg(
                                  filePath),
                              QMessageBox::Ok);
    }

    // Delay quit to let other apps grab the clipboard ownership
    this->hide();
    QTimer::singleShot(60 * 1000, QGuiApplication::instance(), &QGuiApplication::quit);
}

void shotmd::mouseReleaseEvent(QMouseEvent* event)
{
    switch (state)
    {
    case anchorFirst:
        {
            shot(anchor, event->pos());
            break;
        }
    default: ;
    }
}

void shotmd::keyPressEvent(QKeyEvent *event) {
    if (event->key() == Qt::Key_F) {
        // Capture the entire screen
        const QScreen *screen = QGuiApplication::primaryScreen();
        const QRect screenGeometry = screen->geometry();
        shot(screenGeometry.topLeft(), screenGeometry.bottomRight());
    } else {
        QWidget::keyPressEvent(event); // Pass event to base class for unhandled keys
    }
}


shotmd::~shotmd()
{
    delete ui;
}
